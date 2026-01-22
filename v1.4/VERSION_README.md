# DNA v1.4 - Recovery Phrase Fix + Data Validation

**Released**: October 2024  
**Status**: ⚠️ Has encryption bug - DO NOT USE for new deployments

---

## What's New in v1.4

### ✅ Fixed `get_recovery_phrase` Update Chain

Added update chain following to `get_recovery_phrase` (same pattern as v1.3's `get_user_profile`).

### ✅ Added `update_recovery_phrase` Function

Previously missing! This function is required for password changes to update the recovery phrase encryption.

```rust
#[hdk_extern]
pub fn update_recovery_phrase(recovery_phrase: RecoveryPhrase) -> ExternResult<Record> {
    let current_record = get_recovery_phrase(())?
        .ok_or(wasm_error!("No recovery phrase found to update"))?;
    
    let updated_hash = update_entry(
        current_record.action_address().clone(),
        &EntryZomes::IntegrityPrivateData(EntryTypes::RecoveryPhrase(recovery_phrase)),
    )?;
    
    let record = get(updated_hash, GetOptions::default())?
        .ok_or(wasm_error!("Could not find the updated entry"))?;
    
    Ok(record)
}
```

### ✅ Data Validation

Added backend data validation to prevent migration of corrupted data.

---

## ⚠️ CRITICAL BUG: 8-Byte Nonces

**Discovery**: November 2, 2025

v1.4 generates **8-byte nonces** (16 hex characters) instead of the required **12-byte nonces** (24 hex characters) for AES-256-GCM encryption.

**Impact**:
- ❌ ALL v1.4 accounts cannot migrate to v1.5
- ⚠️ Weaker encryption than intended
- ✅ Data validation correctly blocks these migrations

**Example of Bad Data**:
```javascript
{
  encrypted_email: "a1b2c3...",  // 32 chars - too short!
  nonce: "1234567890abcdef",     // 16 chars - SHOULD BE 24!
  salt: "abcdef1234567890...",   // 32 chars - too short!
  tag: "9876543210fedcba..."     // 32 chars - OK
}
```

**Root Cause**: Unknown - likely in registration endpoint or encryption utility function.

**Fixed In**: Not fixed! v1.4 remains unmigrateable. New accounts should be created directly on v1.5.

---

## Known Issues

⚠️ **ENCRYPTION BUG**: Generates short nonces (8 bytes instead of 12 bytes)

⚠️ **Same Update Chain Bug**: Still only follows ONE level of updates, not the entire chain (like v1.3)

**Fixed In**: v1.5 (recursive loop + proper encryption)

---

## Migration Path

- ✅ v1.3 → v1.4 (successful)
- ❌ v1.4 → v1.5 (BLOCKED by nonce bug - data validation prevents migration)

---

## Network Seed

```yaml
network_seed: "flowsta-private-network-v1.4"
```

---

## Build

```bash
cd v1.4
bash build.sh
# Output: workdir/flowsta_private_v1_4_happ.happ
```

---

## Lesson Learned

**ALWAYS test migration with FRESH accounts on the old version!**

We discovered this bug only when trying to migrate freshly created v1.4 accounts to v1.5. The data validation system correctly caught the issue and prevented migration of insecure encryption.

---

**⚠️ DO NOT USE** - Has encryption bug  
**Use v1.5 instead** - All issues fixed

