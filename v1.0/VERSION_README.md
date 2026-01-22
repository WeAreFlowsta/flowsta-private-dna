# DNA v1.0 - Original Version

**Released**: October 2024  
**Status**: Legacy (replaced by v1.5)

---

## What's in This Version

This is the original Flowsta Private DNA implementation with:

- ✅ UserProfile entry type (encrypted email)
- ✅ RecoveryPhrase entry type (encrypted mnemonic)
- ✅ Basic `get_user_profile` function
- ✅ Basic `get_recovery_phrase` function
- ✅ Client-side AES-256-GCM encryption

---

## Known Issues

⚠️ **Update Chain Bug**: The `get_user_profile` and `get_recovery_phrase` functions do not follow the update chain, so they always return the original entry, not the latest updated version.

**Impact**: 
- Password changes appear to work but actually don't update the encrypted data
- Recovery phrases can't be updated

**Fixed In**: v1.3 (for profiles), v1.4 (for recovery phrases)

---

## Migration Path

- ✅ v1.0 → v1.1 (adds EmailPermission)

---

## Entry Types

### UserProfile

```rust
pub struct UserProfile {
    pub encrypted_email: String,
    pub nonce: String,
    pub salt: String,
    pub tag: String,
    pub display_name: Option<String>,
    pub created_at: i64,
}
```

### RecoveryPhrase

```rust
pub struct RecoveryPhrase {
    pub encrypted_mnemonic: String,
    pub nonce: String,
    pub salt: String,
    pub tag: String,
    pub verified: bool,
    pub created_at: i64,
}
```

---

## Network Seed

```yaml
network_seed: "flowsta-private-network"
```

---

## Build

```bash
cd v1.0
bash build.sh
# Output: workdir/flowsta_private_happ.happ
```

---

**Historical Version** - Not recommended for new deployments

