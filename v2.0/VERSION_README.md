# DNA v2.0 — Encrypt-Before-Gossip

**Status**: 🚧 IN DEVELOPMENT

---

## What v2.0 is

v1.x marked entries private-visibility (no gossip) and stored most fields in
plaintext. v2.0 inverts the model for device-hosted identities:

- **One entry type, `Sealed { cipher, nonce }`** — an opaque
  XSalsa20-Poly1305 secretbox blob. Entry type, timestamps, app ids, and
  relationships all live INSIDE the ciphertext (`SealedPayload`, built and
  decrypted in Flowsta Vault). The zome performs zero crypto and zero
  filtering; querying is client-side after decryption.
- **PUBLIC visibility** — records DO gossip, so a user's devices replicate
  each other's data. Only ciphertext ever leaves a device.
- **One link type (`AgentToSealed`), empty tags** — no per-type link counts
  or tag contents to leak activity categories (v1.x leaked `app_id` in a
  link tag and category volumes via 11 typed links).
- **Per-user network seed, derived from the recovery phrase** — supplied at
  install time (`flowsta-vault key_derivation::derive_private_network_seed`).
  The dna.yaml seed is a placeholder; installs MUST override it. Every
  device of one user derives the same seed (same private network, zero
  coordination); different users are in disjoint networks.
- **Encryption key**: per-user symmetric
  `data_key = HMAC-SHA256('flowsta-data-encryption-v1', bip39_seed)` —
  phrase-derived so every device decrypts and recovery needs only the
  phrase. (NOT crypto_box-by-agent-key: per-device agent keys differ, which
  would make records unreadable across devices.)

## What v2.0 deliberately does NOT have

- **RecoveryPhrase entries** — the phrase never enters a cell for
  device-hosted users (it IS the identity root, held in Vault).
- **TotpConfig entries** — 2FA config moves Vault-local.
- **Any plaintext-filtering zome functions** — `get_analytics_id_for_app`,
  `get_oauth_activity_by_app` etc. have no v2 equivalents; the client
  decrypts and filters locally.

## Zome API

| Function | Purpose |
|---|---|
| `create_sealed({cipher, nonce})` | store + link a sealed record |
| `get_all_sealed()` | all live records (ciphertext) for this agent |
| `delete_sealed(action_hash)` | tombstone + link cleanup (author-only) |
| `replace_sealed({original_hash, replacement})` | supersede (create new, tombstone old) |
| `export_all_data()` | full-state ciphertext export (CAL/backup/migration) |

Validation: authorship-only, content-blind (same rules as v1.11).

## Deployment model

v2 is **device-only**: installed by Flowsta Vault on user devices with the
per-user seed override. The API conductor never hosts a v2 cell, and there
is no server-orchestrated migration for it. Migration of v1.11 data happens
client-side in Vault (export from the API cell → decrypt → wrap into
`SealedPayload` → `create_sealed` into the device cell).

## Build

```bash
./build.sh   # wasm32 build + hc dna pack + hc app pack → workdir/
```
