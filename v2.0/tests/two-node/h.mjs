// Two-node harness for the private v2 network. Usage: node h.mjs <setup|phase1|swap|phase2|restartcheck|status>
import fs from 'fs';
import crypto from 'crypto';
// @holochain/client 0.20.x; set HC_CLIENT to its lib/index.js (any checkout that has it installed).
const hc = await import(process.env.HC_CLIENT);
const { AdminWebsocket, AppWebsocket, encodeHashToBase64 } = hc;
const SP = new URL('.', import.meta.url).pathname;
const V2 = new URL('../../', import.meta.url).pathname;
const STATE = SP + 'state.json';
const HAPP = V2 + 'workdir/flowsta_private_v2_0_happ.happ';   // the shipped bundle, never rebuilt
const NEW_COORD = V2 + 'target/wasm32-unknown-unknown/release/private_data_coordinator.wasm';
const ROLE = 'flowsta_private_v2_0', ZOME = 'private_data', APP = 'priv';
const PORTS = { a: 46001, b: 46002, c: 46003 };
const origin = 'flowsta-vault';
const sleep = (ms) => new Promise(r => setTimeout(r, ms));
const b64 = (h) => encodeHashToBase64(h);
const load = () => JSON.parse(fs.readFileSync(STATE, 'utf8'));
const save = (s) => fs.writeFileSync(STATE, JSON.stringify(s, null, 1));
const rev = (o) => hc.decodeHashFromBase64(o);
const sealed = (n = 64) => ({ cipher: crypto.randomBytes(n), nonce: crypto.randomBytes(24) });
const now = () => Date.now();

async function admin(n) { return AdminWebsocket.connect({ url: new URL(`ws://localhost:${PORTS[n]}`), wsClientOptions: { origin } }); }
function cellOf(app) { const c = app.cell_info[ROLE].find(c => c.type === 'provisioned'); return c.value.cell_id; }
async function node(n) {
  const ad = await admin(n);
  const app = (await ad.listApps({})).find(a => a.installed_app_id === APP);
  const cell_id = cellOf(app);
  let ifaces = await ad.listAppInterfaces();
  let port = ifaces[0]?.port;
  if (!port) port = (await ad.attachAppInterface({ allowed_origins: '*' })).port;
  await ad.authorizeSigningCredentials(cell_id);
  const tok = await ad.issueAppAuthenticationToken({ installed_app_id: APP });
  const ws = await AppWebsocket.connect({ url: new URL(`ws://localhost:${port}`), token: tok.token, wsClientOptions: { origin } });
  const call = (fn_name, payload) => ws.callZome({ role_name: ROLE, zome_name: ZOME, fn_name, payload }, 60000);
  return { n, ad, ws, cell_id, agent: cell_id[1], dna: cell_id[0], call };
}
const tryCall = async (f) => { try { return { ok: await f() }; } catch (e) { return { err: String(e?.message || e).replace(/\s+/g, ' ').slice(0, 260) }; } };
async function until(label, f, timeoutS = 600, everyMs = 3000) {
  const t0 = now();
  for (;;) { const v = await f(); if (v) { const s = ((now() - t0) / 1000).toFixed(0); console.log(`   ${label}: yes after ${s}s`); return Number(s); }
    if ((now() - t0) / 1000 > timeoutS) { console.log(`   ${label}: NOT within ${timeoutS}s`); return null; } await sleep(everyMs); }
}

const cmd = process.argv[2];
if (cmd === 'setup') {
  const seed = 'spike-' + crypto.randomBytes(8).toString('hex');
  const st = { seed, nodes: {} };
  for (const n of ['a', 'b']) {
    const ad = await admin(n);
    const agent_key = await ad.generateAgentPubKey();
    const app = await ad.installApp({ source: { type: 'path', value: HAPP }, agent_key, installed_app_id: APP, network_seed: seed });
    await ad.enableApp({ installed_app_id: APP });
    const cell_id = cellOf((await ad.listApps({})).find(a => a.installed_app_id === APP));
    st.nodes[n] = { agent: b64(cell_id[1]), dna: b64(cell_id[0]) };
    console.log(`${n}: agent ${st.nodes[n].agent.slice(0, 16)}… dna ${st.nodes[n].dna.slice(0, 16)}…`);
  }
  console.log('same network (dna hash equal):', st.nodes.a.dna === st.nodes.b.dna, '| different agents:', st.nodes.a.agent !== st.nodes.b.agent);
  save(st);
}
if (cmd === 'phase1') {
  const st = load(); const A = await node('a'), B = await node('b');
  console.log('SHIPPED coordinator. A writes 5 records.');
  const recs = []; for (let i = 0; i < 5; i++) recs.push(await A.call('create_sealed', sealed()));
  st.a_records = recs.map(r => b64(r.signed_action.hashed.hash)); st.a_written_at = now(); save(st);
  console.log('   A lists (get_all_sealed):', (await A.call('get_all_sealed', null)).length);
  console.log('   B lists (get_all_sealed):', (await B.call('get_all_sealed', null)).length, ' <- the audit said 0');
  const peers = async (X) => (await X.ad.agentInfo({ dna_hashes: [X.dna] })).length;
  await until('A and B know each other (agent infos >= 2 on both)', async () => (await peers(A)) >= 2 && (await peers(B)) >= 2, 300);
}
if (cmd === 'swap') {
  const st = load();
  const wasm = new Uint8Array(fs.readFileSync(NEW_COORD));
  const bundle = { manifest: { zomes: [{ name: ZOME, path: 'private_data_coordinator.wasm', dependencies: [{ name: 'private_data_integrity' }] }] }, resources: { 'private_data_coordinator.wasm': wasm } };
  for (const n of ['a', 'b']) {
    const ad = await admin(n);
    const before = cellOf((await ad.listApps({})).find(a => a.installed_app_id === APP));
    await ad.updateCoordinators({ cell_id: before, source: { type: 'bundle', value: bundle } });
    const after = cellOf((await ad.listApps({})).find(a => a.installed_app_id === APP));
    console.log(`${n}: coordinator swapped | dna hash unchanged: ${b64(before[0]) === b64(after[0])} | still ${st.nodes[n].dna === b64(after[0])}`);
  }
}
if (cmd === 'phase2') {
  const st = load(); const A = await node('a'), B = await node('b');
  const base = A.agent;   // stands in for the identity key: the base existing records already hang from
  const listAt = (X, extra = {}) => X.call('get_all_sealed_at', { base, ...extra });
  console.log('NEW coordinator.');
  console.log('1. Old records through the new function, on the device that wrote them: A lists', (await listAt(A)).length, '(want 5)');
  const t = await until('2. B HOLDS A\'s 5 records locally (no network read)', async () => (await listAt(B)).length >= 5, 900);
  if (t !== null) console.log(`   (since A wrote them: ${((now() - st.a_written_at) / 1000).toFixed(0)}s)`);
  const viaNet = await tryCall(() => listAt(B, { network: true })); console.log('   B via a network read:', viaNet.ok ? viaNet.ok.length : viaNet.err);
  const [r0, r1, r2] = st.a_records.map(rev);
  console.log('3. B deletes A\'s ENTRY outright:'); const del = await tryCall(() => B.call('delete_sealed', r0)); console.log('   ->', del.err ? 'REFUSED: ' + del.err : 'ACCEPTED (unexpected) ' + b64(del.ok));
  console.log('   A\'s record still listed on B:', (await listAt(B)).some(x => b64(x.record.signed_action.hashed.hash) === st.a_records[0]));
  console.log('4. B retires A\'s record by removing its link:'); const ret = await tryCall(() => B.call('retire_sealed_at', { base, target: r1 })); console.log('   ->', ret.err ? 'FAILED: ' + ret.err : `links removed: ${ret.ok}`);
  console.log('   B now lists', (await listAt(B)).length, '(want 4)');
  await until('   A sees the retirement (A lists 4)', async () => (await listAt(A)).length === 4, 600);
  console.log('   A\'s OLD function (what a 1.5.0 Vault calls) lists', (await A.call('get_all_sealed', null)).length);
  console.log('5. B writes a record at the shared base:'); const bRec = await B.call('create_sealed_at', { base, ...sealed(), tag: new Uint8Array() }); const bHash = b64(bRec.signed_action.hashed.hash); const tB = now();
  await until('   A sees B\'s record through the new function', async () => (await listAt(A)).some(x => b64(x.record.signed_action.hashed.hash) === bHash), 600);
  console.log(`   (${((now() - tB) / 1000).toFixed(0)}s after B wrote it)`);
  const oldSees = (await A.call('get_all_sealed', null)).some(r => b64(r.signed_action.hashed.hash) === bHash); console.log('   A\'s OLD function also lists B\'s record:', oldSees);
  console.log('6. B supersedes A\'s record (replace_sealed_at):'); const rep = await tryCall(() => B.call('replace_sealed_at', { original: r2, replacement: { base, ...sealed(), tag: new Uint8Array() } })); console.log('   ->', rep.err ? 'FAILED: ' + rep.err : 'ok; B lists ' + (await listAt(B)).length + ' (want 5: 3 of A\'s + B\'s 2)');
  console.log('7. A\'s OLD replace on a record B wrote (what a 1.5.0 Vault would do):'); const oldRep = await tryCall(() => A.call('replace_sealed', { original_hash: rev(bHash), replacement: sealed() })); console.log('   ->', oldRep.err ? 'REFUSED: ' + oldRep.err : 'accepted');
  console.log('8. Marker with tag "d":'); await B.call('create_sealed_at', { base, cipher: Buffer.from(JSON.stringify({ kind: 'device', name: 'spike marker, plaintext' })), nonce: crypto.randomBytes(24), tag: Buffer.from('d') });
  console.log('   markers listed:', (await listAt(B, { tag_prefix: Buffer.from('d') })).length, '| records listed (marker excluded):', (await listAt(B)).length);
  st.after_phase2 = { b_count: (await listAt(B)).length }; save(st);
}

if (cmd === 'phase3') {
  const st = load(); const A = await node('a'), B = await node('b'); const base = A.agent;
  const listAt = (X, extra = {}) => X.call('get_all_sealed_at', { base, ...extra });
  const hashes = async (X) => (await listAt(X)).map(x => b64(x.record.signed_action.hashed.hash));
  const [r0, r1, r2] = st.a_records.map(rev);
  const before = (await listAt(B)).length; console.log('B lists', before, 'records before');
  console.log('3. B deletes A\'s ENTRY outright:'); const del = await tryCall(() => B.call('delete_sealed', r0)); console.log('   ->', del.err ? 'REFUSED: ' + del.err : 'ACCEPTED by B\'s own node: ' + b64(del.ok));
  console.log('   still listed on B right after:', (await hashes(B)).includes(st.a_records[0]));
  await sleep(45000);
  console.log('   45s later - listed on B:', (await hashes(B)).includes(st.a_records[0]), '| listed on A:', (await hashes(A)).includes(st.a_records[0]));
  console.log('4. B retires A\'s record by removing its link:'); const ret = await tryCall(() => B.call('retire_sealed_at', { base, target: r1 })); console.log('   ->', ret.err ? 'FAILED: ' + ret.err : `links removed: ${ret.ok}`);
  console.log('   B lists it:', (await hashes(B)).includes(st.a_records[1]));
  await until('   A no longer lists it', async () => !(await hashes(A)).includes(st.a_records[1]), 300);
  console.log('   A\'s OLD function (a 1.5.0 Vault) lists it:', (await A.call('get_all_sealed', null)).some(r => b64(r.signed_action.hashed.hash) === st.a_records[1]));
  console.log('6. B supersedes A\'s record (replace_sealed_at):'); const rep = await tryCall(() => B.call('replace_sealed_at', { original: r2, replacement: { base, ...sealed(), tag: new Uint8Array() } }));
  console.log('   ->', rep.err ? 'FAILED: ' + rep.err : 'ok');
  if (rep.ok) { const nh = b64(rep.ok.signed_action.hashed.hash); await until('   A lists the replacement and not the original', async () => { const h = await hashes(A); return h.includes(nh) && !h.includes(st.a_records[2]); }, 300); st.b_replacement = nh; }
  console.log('7. A\'s OLD replace on a record B wrote (what a 1.5.0 Vault would do):');
  const bRec = (await listAt(A)).find(x => b64(x.record.signed_action.hashed.content.author) === b64(B.agent));
  const oldRep = await tryCall(() => A.call('replace_sealed', { original_hash: bRec.record.signed_action.hashed.hash, replacement: sealed() })); console.log('   ->', oldRep.err ? 'REFUSED: ' + oldRep.err : 'accepted by A\'s own node');
  console.log('   A lists', (await listAt(A)).length, '| A old function lists', (await A.call('get_all_sealed', null)).length, '| B lists', (await listAt(B)).length);
  save(st);
}

if (cmd === 'step7') {
  const A = await node('a'), B = await node('b'); const base = A.agent;
  const listAt = (X) => X.call('get_all_sealed_at', { base });
  const bRec = (await listAt(A)).find(x => b64(x.record.signed_action.hashed.content.author) === b64(B.agent));
  console.log('7. A\'s OLD replace on a record B wrote (what a 1.5.0 Vault would do):');
  const oldRep = await tryCall(() => A.call('replace_sealed', { original_hash: bRec.record.signed_action.hashed.hash, replacement: sealed() })); console.log('   ->', oldRep.err ? 'REFUSED: ' + oldRep.err : 'accepted by A\'s own node');
  console.log('   A lists', (await listAt(A)).length, '| A old function lists', (await A.call('get_all_sealed', null)).length, '| B lists', (await listAt(B)).length);
}

if (cmd === 'bulk') {
  // A writes a realistic cell: many small records + a few large ones.
  const st = load(); const A = await node('a'); const base = A.agent; const t0 = now();
  const N = Number(process.argv[3] || 500);
  for (let i = 0; i < N; i++) await A.call('create_sealed_at', { base, ...sealed(1500 + (i % 7) * 400), tag: new Uint8Array() });
  for (let i = 0; i < 3; i++) await A.call('create_sealed_at', { base, ...sealed(800 * 1000), tag: new Uint8Array() });
  const n = (await A.call('get_all_sealed_at', { base })).length;
  console.log(`A wrote ${N} small + 3 large (800 KB) records in ${((now() - t0) / 1000).toFixed(0)}s; A lists ${n}`);
  st.bulk_total = n; save(st);
  const B = await node('b');
  await until(`B holds all ${n}`, async () => (await B.call('get_all_sealed_at', { base })).length >= n, 1500, 5000);
}
if (cmd === 'join') {
  // A brand-new device joins the existing network and catches up from scratch.
  const st = load(); const ad = await admin('c'); const t0 = now();
  const agent_key = await ad.generateAgentPubKey();
  await ad.installApp({ source: { type: 'path', value: HAPP }, agent_key, installed_app_id: APP, network_seed: st.seed });
  await ad.enableApp({ installed_app_id: APP });
  const cell = cellOf((await ad.listApps({})).find(a => a.installed_app_id === APP));
  const wasm = new Uint8Array(fs.readFileSync(NEW_COORD));
  await ad.updateCoordinators({ cell_id: cell, source: { type: 'bundle', value: { manifest: { zomes: [{ name: ZOME, path: 'private_data_coordinator.wasm', dependencies: [{ name: 'private_data_integrity' }] }] }, resources: { 'private_data_coordinator.wasm': wasm } } } });
  console.log('C installed + coordinator applied, same network:', b64(cell[0]) === st.nodes.a.dna, `(${((now() - t0) / 1000).toFixed(0)}s)`);
  const C = await node('c'); const base = rev(st.nodes.a.agent); let last = -1;
  await until(`C holds all ${st.bulk_total}`, async () => { const n = (await C.call('get_all_sealed_at', { base })).length; if (n !== last) { console.log(`     ${((now() - t0) / 1000).toFixed(0)}s: ${n}`); last = n; } return n >= st.bulk_total; }, 1700, 5000);
  const t1 = now(); const listed = await C.call('get_all_sealed_at', { base }); console.log(`   listing ${listed.length} records locally on C takes ${now() - t1} ms`);
}
if (cmd === 'restartcheck') {
  const st = load(); const A = await node('a'), B = await node('b'); const base = A.agent;
  const rec = await A.call('create_sealed_at', { base, ...sealed(), tag: new Uint8Array() }); const h = b64(rec.signed_action.hashed.hash);
  console.log('After both restarted: A wrote a new record.');
  await until('B holds it', async () => (await B.call('get_all_sealed_at', { base })).some(x => b64(x.record.signed_action.hashed.hash) === h), 900);
}
if (cmd === 'status') {
  for (const n of ['a', 'b']) { const X = await node(n); const infos = await X.ad.agentInfo({ dna_hashes: [X.dna] }); const stats = await X.ad.dumpNetworkStats();
    console.log(n, 'agent infos:', infos.length, '| connections:', stats.connections?.length ?? JSON.stringify(stats).slice(0, 200)); }
}
process.exit(0);
