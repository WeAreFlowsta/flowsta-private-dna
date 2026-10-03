// Two machines, one private network. Runs the same on Linux, macOS and Windows.
//   node two.mjs host                      first machine: writes records, prints the line for the second
//   node two.mjs join <seed> <base>        second machine: joins, catches up, then both exchange a large object
// Finds the holochain binary of an installed Flowsta Vault (or set HC) and the staging rendezvous auth material (or set AUTH / auth.txt).
import fs from 'fs'; import os from 'os'; import path from 'path'; import crypto from 'crypto'; import { spawn } from 'child_process';
import { AdminWebsocket, AppWebsocket, encodeHashToBase64 as b64, decodeHashFromBase64 as unb64 } from '@holochain/client';
import { decode } from '@msgpack/msgpack';

const here = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const role = process.argv[2];
const opt = (name, dflt) => { const i = process.argv.indexOf('--' + name); return i > 0 ? process.argv[i + 1] : dflt; };
const RECORDS = Number(opt('records', 500)), SEND_MB = Number(opt('send-mb', 50)), PIECE_KB = Number(opt('piece-kb', 512)), WINDOW = Number(opt('window', 4));
const PORT = role === 'host' ? 46011 : 46012;
const DATA = opt('data', path.join(os.platform() === 'win32' ? os.tmpdir() : '/tmp', `fvsp-${role}`));   // short path: the key store's socket has a length limit
const BOOT = process.env.BOOT || 'bootstrap-staging.flowsta.com';
const exe = os.platform() === 'win32' ? '.exe' : '';
// The holochain binary: HC, else the one an installed Flowsta Vault carries, else a Vault checkout beside this repo.
const vaultRepo = path.resolve(here, '../../../../flowsta-vault');
const hcCandidates = [process.env.HC,
  process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, 'Flowsta Vault', 'vault-holochain.exe'),
  process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, 'Programs', 'Flowsta Vault', 'vault-holochain.exe'),
  process.env.ProgramFiles && path.join(process.env.ProgramFiles, 'Flowsta Vault', 'vault-holochain.exe'),
  '/Applications/Flowsta Vault.app/Contents/MacOS/vault-holochain',
  '/usr/bin/vault-holochain',
  path.join(vaultRepo, 'src-tauri/binaries/vault-holochain-x86_64-unknown-linux-gnu'),
  path.join(vaultRepo, 'src-tauri/binaries/vault-holochain-x86_64-pc-windows-msvc.exe'),
].filter(Boolean);
const HC_SRC = hcCandidates.find(p => fs.existsSync(p));
// The rendezvous auth material for staging: AUTH, else auth.txt here, else the Vault repo's own test script (local checkout, or the public repo).
async function resolveAuth() {
  if (process.env.AUTH) return process.env.AUTH;
  if (fs.existsSync(path.join(here, 'auth.txt'))) return fs.readFileSync(path.join(here, 'auth.txt'), 'utf8').trim();
  const pick = (t) => (t.match(/FLOWSTA_AUTH_MATERIAL=([A-Za-z0-9+/=]+)/) || [])[1] || '';
  const local = path.join(vaultRepo, 'scripts/run-test-instance.sh');
  if (fs.existsSync(local)) return pick(fs.readFileSync(local, 'utf8'));
  try { return pick(await (await fetch('https://raw.githubusercontent.com/WeAreFlowsta/flowsta-vault-app/main/scripts/run-test-instance.sh')).text()); } catch { return ''; }
}
const AUTH = await resolveAuth();
const HAPP = path.join(here, 'fixtures', 'flowsta_private_v2_0_happ.happ'), COORD = path.join(here, 'fixtures', 'private_data_coordinator.wasm');
const ROLE = 'flowsta_private_v2_0', ZOME = 'private_data', APP = 'priv', origin = 'flowsta-vault';
const sleep = (ms) => new Promise(r => setTimeout(r, ms)); const now = () => Date.now(); const t0 = now();
const log = (...a) => console.log(`[${String(((now() - t0) / 1000).toFixed(0)).padStart(4)}s]`, ...a);
if (!['host', 'join'].includes(role)) { console.error('usage: node two.mjs host | join <seed> <base>'); process.exit(1); }
if (!HC_SRC) { console.error('Could not find the holochain binary. Install Flowsta Vault, or set HC to the path of vault-holochain' + exe); process.exit(1); }
if (!AUTH) { console.error('Could not find the staging auth material. Put it in auth.txt beside this file, or set AUTH.'); process.exit(1); }

// ── conductor ──
fs.mkdirSync(path.join(DATA, 'data'), { recursive: true }); fs.mkdirSync(path.join(DATA, 'ks'), { recursive: true });
const q = (p) => `'${p.replace(/'/g, "''")}'`;
fs.writeFileSync(path.join(DATA, 'conductor-config.yaml'), `data_root_path: ${q(path.join(DATA, 'data'))}
keystore:
  type: lair_server_in_proc
  lair_root: ${q(path.join(DATA, 'ks'))}
admin_interfaces:
- driver:
    type: websocket
    port: ${PORT}
    allowed_origins: '*'
network:
  bootstrap_url: https://${BOOT}
  signal_url: wss://${BOOT}
  relay_url: https://${BOOT}./
  base64_auth_material_bootstrap: "${AUTH}"
  base64_auth_material_relay: "${AUTH}"
  request_timeout_s: 240
`);
// Run a copy under its own name, so a Flowsta Vault started meanwhile does not take it for one of its own leftovers.
const HC = path.join(DATA, 'private-network-check' + exe);
fs.copyFileSync(HC_SRC, HC); if (!exe) fs.chmodSync(HC, 0o755);
const child = spawn(HC, ['--piped', '-c', path.join(DATA, 'conductor-config.yaml')], { stdio: ['pipe', 'pipe', 'pipe'] });
child.stdin.write('two-node-check\n'); child.stdin.end();
const logFile = fs.createWriteStream(path.join(DATA, 'holochain.log'), { flags: 'a' });
let ready = false; child.stdout.on('data', d => { logFile.write(d); if (String(d).includes('Conductor ready')) ready = true; }); child.stderr.on('data', d => logFile.write(d));
child.on('exit', (c) => { log(`conductor exited (${c}); see ${path.join(DATA, 'holochain.log')}`); process.exit(1); });
const stop = () => { try { child.kill(); } catch {} process.exit(0); }; process.on('SIGINT', stop); process.on('SIGTERM', stop); process.on('SIGHUP', stop); process.on('exit', () => { try { child.kill(); } catch {} }); process.stdout.on('error', stop);
for (let i = 0; i < 60 && !ready; i++) await sleep(1000);
if (!ready) { log('conductor did not start'); stop(); }
log(`conductor up (${os.platform()}), data in ${DATA}`);

// ── app ──
const admin = await AdminWebsocket.connect({ url: new URL(`ws://localhost:${PORT}`), wsClientOptions: { origin } });
const cellOf = (app) => app.cell_info[ROLE].find(c => c.type === 'provisioned').value.cell_id;
let app = (await admin.listApps({})).find(a => a.installed_app_id === APP);
const seed = role === 'host' ? (app ? fs.readFileSync(path.join(DATA, 'seed'), 'utf8') : 'two-' + crypto.randomBytes(8).toString('hex')) : process.argv[3];
if (!app) {
  const agent_key = await admin.generateAgentPubKey();
  await admin.installApp({ source: { type: 'path', value: HAPP }, agent_key, installed_app_id: APP, network_seed: seed });
  await admin.enableApp({ installed_app_id: APP }); fs.writeFileSync(path.join(DATA, 'seed'), seed);
  app = (await admin.listApps({})).find(a => a.installed_app_id === APP);
}
const cell_id = cellOf(app), me = cell_id[1];
await admin.updateCoordinators({ cell_id, source: { type: 'bundle', value: { manifest: { zomes: [{ name: ZOME, path: 'c.wasm', dependencies: [{ name: 'private_data_integrity' }] }] }, resources: { 'c.wasm': new Uint8Array(fs.readFileSync(COORD)) } } } });
const port = (await admin.listAppInterfaces())[0]?.port || (await admin.attachAppInterface({ allowed_origins: '*' })).port;
await admin.authorizeSigningCredentials(cell_id);
const ws = await AppWebsocket.connect({ url: new URL(`ws://localhost:${port}`), token: (await admin.issueAppAuthenticationToken({ installed_app_id: APP })).token, wsClientOptions: { origin } });
const call = (fn_name, payload) => ws.callZome({ role_name: ROLE, zome_name: ZOME, fn_name, payload }, 120000);
await call('ensure_piece_grant', null);
const base = role === 'host' ? me : unb64(process.argv[4]);
const sealed = (n) => ({ base, cipher: crypto.randomBytes(n), nonce: crypto.randomBytes(24), tag: new Uint8Array() });
const marker = (tag, obj) => call('create_sealed_at', { base, cipher: Buffer.from(JSON.stringify(obj).padEnd(20)), nonce: crypto.randomBytes(24), tag: Buffer.from(tag) });
const markers = async (tag) => (await call('get_all_sealed_at', { base, tag_prefix: Buffer.from(tag) })).map(x => { try { return JSON.parse(Buffer.from(decode(x.record.entry.Present.entry).cipher).toString()); } catch { return null; } }).filter(Boolean);
const count = async () => (await call('get_all_sealed_at', { base })).length;
const net = async () => { try { const s = await admin.dumpNetworkStats(); const c = s.transport_stats?.connections || []; return `${c.length} connection(s)` + (c.length ? `, direct: ${c.map(x => x.is_direct ?? '?').join('/')}` : ''); } catch (e) { return 'stats unavailable'; } };
const peers = async () => (await admin.agentInfo({ dna_hashes: [cell_id[0]] })).length;
log(`network seed ${seed} | this device ${b64(me).slice(0, 14)}… | same network check: dna ${b64(cell_id[0]).slice(0, 14)}…`);

// ── large objects, in pieces ──
const incoming = new Map(); let peerAgent = null; const acks = new Map();
ws.on('signal', async (sig) => {
  const { from, piece } = sig.value?.payload ?? sig.payload ?? {}; if (!piece) return; peerAgent = from;
  if (piece.kind === 'ack') { acks.get(piece.id)?.(piece.seq); return; }
  if (piece.kind === 'hello') { log(`the other device said hello (${Buffer.from(piece.data).toString()})`); return; }
  if (piece.kind !== 'data') return;
  let t = incoming.get(piece.id); if (!t) { t = { parts: new Array(piece.total), got: 0, started: now(), dup: 0 }; incoming.set(piece.id, t); log(`receiving a large object in ${piece.total} pieces…`); }
  if (t.parts[piece.seq]) t.dup++; else { t.parts[piece.seq] = Buffer.from(piece.data); t.got++; }
  call('send_piece', { to: from, piece: { id: piece.id, kind: 'ack', seq: piece.seq, total: piece.total, data: new Uint8Array() } }).catch(() => {});
  if (t.got === piece.total && !t.done) { t.done = true; const all = Buffer.concat(t.parts); const ok = crypto.createHash('sha256').update(all).digest('hex') === piece.id; const s = (now() - t.started) / 1000;
    log(`RECEIVED ${(all.length / 1e6).toFixed(0)} MB in ${s.toFixed(0)}s (${(all.length / 1e6 / s).toFixed(2)} MB/s), content hash ${ok ? 'matches' : 'DOES NOT MATCH'}, duplicate pieces ${t.dup} | ${await net()}`); }
});
async function sendLarge(to, mb) {
  const data = crypto.randomBytes(mb * 1000 * 1000), id = crypto.createHash('sha256').update(data).digest('hex'), size = PIECE_KB * 1000, total = Math.ceil(data.length / size);
  const acked = new Set(); let resends = 0; acks.set(id, (seq) => acked.add(seq)); const started = now();
  log(`sending ${mb} MB in ${total} pieces of ${PIECE_KB} KB, ${WINDOW} at a time…`);
  const sendOne = (seq) => call('send_piece', { to, piece: { id, kind: 'data', seq, total, data: data.subarray(seq * size, (seq + 1) * size) } });
  let next = 0; const inflight = new Map();
  while (acked.size < total) {
    for (const [seq, at] of inflight) { if (acked.has(seq)) inflight.delete(seq); else if (now() - at > 20000) { resends++; inflight.set(seq, now()); await sendOne(seq).catch(e => log('send error', String(e).slice(0, 120))); } }
    while (inflight.size < WINDOW && next < total) { inflight.set(next, now()); await sendOne(next).catch(e => log('send error', String(e).slice(0, 120))); next++; }
    await sleep(25);
    if ((now() - started) > 30 * 60 * 1000) { log(`GAVE UP after 30 min: ${acked.size}/${total} acknowledged`); return; }
  }
  const s = (now() - started) / 1000; log(`SENT ${mb} MB in ${s.toFixed(0)}s (${(mb / s).toFixed(2)} MB/s), pieces resent ${resends} | ${await net()}`);
}

// ── the run ──
if (role === 'host') {
  if ((await count()) < RECORDS) { log(`writing ${RECORDS} small and 3 large records…`); for (let i = 0; i < RECORDS; i++) await call('create_sealed_at', sealed(1500 + (i % 7) * 400)); for (let i = 0; i < 3; i++) await call('create_sealed_at', sealed(800 * 1000)); }
  const total = await count(); await marker('n', { total });
  console.log(`\n  On the second machine run:\n\n    node two.mjs join ${seed} ${b64(me)}\n`);
  let last = total, tick = 0;
  for (;;) {
    await sleep(30000); tick++;
    await marker('t', { t: now(), tick });
    const c = await count(); if (c !== last) { log(`this device now lists ${c} records (was ${last}) - the other device's records arrived`); last = c; }
    if (tick % 4 === 0) log(`still here: ${c} records, ${await peers()} device(s) known, ${await net()}`);
  }
} else {
  let total = null, last = -1; const joined = now();
  for (;;) {
    const c = await count(); if (total === null) total = (await markers('n'))[0]?.total ?? null;
    if (c !== last) { log(`holding ${c}${total ? ' of ' + total : ''} records | ${await net()}`); last = c; }
    if (total && c >= total) break; await sleep(4000);
  }
  log(`CAUGHT UP: all ${total} records ${((now() - joined) / 1000).toFixed(0)}s after joining`);
  for (let i = 0; i < 3; i++) await call('create_sealed_at', sealed(2000)); log('wrote 3 records here - the first machine should report them within a minute');
  // find the other device and exchange a large object both ways
  for (let i = 0; i < 30 && !peerAgent; i++) { await call('send_piece', { to: base, piece: { id: 'hello', kind: 'hello', seq: 0, total: 1, data: Buffer.from(os.platform()) } }).catch(() => {}); await sleep(1000); if (i === 0) peerAgent = base; }
  await sendLarge(base, SEND_MB);
  log('Now the sleep test: put this machine to sleep for 3-5 minutes, wake it, and watch the lines below.');
  let seen = 0;
  for (;;) {
    const ticks = await markers('t'); const newest = ticks.reduce((m, x) => Math.max(m, x.tick || 0), 0), latest = ticks.find(x => x.tick === newest);
    if (newest !== seen) { log(`heartbeat ${newest} from the first machine arrived ${latest ? ((now() - latest.t) / 1000).toFixed(0) : '?'}s after it was written | ${await net()}`); seen = newest; }
    await sleep(5000);
  }
}
