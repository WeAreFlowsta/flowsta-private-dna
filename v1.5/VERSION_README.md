# DNA v1.5 - Recursive Update Chain Fix

**Released**: November 2, 2025  
**Status**: ✅ **CURRENT PRODUCTION VERSION**

---

## What's Fixed in v1.5

### ✅ CRITICAL: Recursive Update Chain Following

**The proper fix for the update chain bug that plagued v1.2-v1.4!**

All `get_*` functions now use a **loop** to recursively follow the **ENTIRE** update chain, not just the last update.

```rust
pub fn get_user_profile(_: ()) -> ExternResult<Option<Record>> {
    let links = get_links(...)?;
    if let Some(link) = links.first() {
        let mut current_hash = ActionHash::try_from(link.target.clone())?;
        
        // ✅ CRITICAL: Loop to follow ENTIRE update chain
        loop {
            let details = get_details(current_hash.clone(), GetOptions::default())?
                .ok_or(wasm_error!("Not found in chain"))?;
            
            match details {
                Details::Record(record_details) => {
                    // Check if there's an update
                    if let Some(latest_update) = record_details.updates.last() {
                        // Continue following the chain
                        current_hash = latest_update.action_address().clone();
                    } else {
                        // No more updates - this is the latest record
                        return Ok(Some(record_details.record));
                    }
                }
                _ => return Err(wasm_error!("Expected Record details")),
            }
        }
    }
    Ok(None)
}
```

### ✅ Applied to ALL Getters

- `get_user_profile()` - Recursive loop ✅
- `get_recovery_phrase()` - Recursive loop ✅
- `get_email_permission()` - Recursive loop ✅

---

## What This Fixes

### Before v1.5 (Bug)

```
Password Change #1: ✅ Works
Password Change #2: ❌ Fails (returns stale data from update #1)
Password Change #3: ❌ Fails (still returns update #1)
```

**Why**: v1.2-v1.4 only followed `.last()` once, getting update #1 but not continuing to update #2, #3, etc.

### After v1.5 (Fixed)

```
Password Change #1: ✅ Works
Password Change #2: ✅ Works
Password Change #3: ✅ Works
Password Change #N: ✅ Works (follows entire chain!)
```

**Why**: Loop continues until no more updates are found, guaranteeing we get the latest version.

---

## Known Issues

⚠️ **Migration from v1.4 Blocked**: Cannot migrate v1.4 accounts due to v1.4's encryption bug (8-byte nonces). Fresh v1.5 accounts work perfectly.

**Workaround**: Create new accounts directly on v1.5 (not an issue for new users).

---

## Testing Requirements

Before deploying v1.6 (next version):

1. ✅ Create fresh test accounts on v1.5
2. ✅ Test password change 3+ times
3. ✅ Test recovery phrase survives password changes
4. ✅ Deploy v1.6 to staging
5. ✅ Attempt migration from v1.5 to v1.6
6. ✅ Verify migration succeeds with valid data
7. ✅ Test password change 3+ times on v1.6

**DO NOT SKIP MIGRATION TESTING!** v1.4 taught us this lesson.

---

## Migration Path

- ✅ Fresh v1.5 accounts (new registrations)
- ❌ v1.4 → v1.5 (blocked by v1.4 nonce bug)
- 🔄 v1.5 → v1.6 (planned for pre-launch testing)

---

## Network Seed

```yaml
network_seed: "flowsta-private-network-v1.5"
```

---

## Build

```bash
cd v1.5
bash build.sh
# Output: workdir/flowsta_private_v1_5_happ.happ
```

---

## Production Deployment

### Current Status (November 2025)

- **Production**: v1.5 (all new registrations)
- **Staging**: v1.5 (testing background migrations)
- **Test Accounts**: Created on v1.5 for v1.6 migration testing

### Verified Working

- ✅ New user registration
- ✅ Recovery phrase setup
- ✅ Password changes (tested 3+ times)
- ✅ Recovery phrase survives password changes
- ✅ Email permissions grant/revoke
- ✅ Background migration system
- ✅ SDK migration polling

---

## Entry Types

### UserProfile

```rust
pub struct UserProfile {
    pub encrypted_email: String,  // AES-256-GCM with 12-byte nonce
    pub nonce: String,             // 24 hex chars (12 bytes)
    pub salt: String,              // 32+ hex chars (16+ bytes)
    pub tag: String,               // 32 hex chars (16 bytes)
    pub display_name: Option<String>,
    pub created_at: i64,
}
```

### RecoveryPhrase

```rust
pub struct RecoveryPhrase {
    pub encrypted_mnemonic: String,
    pub nonce: String,              // 24 hex chars
    pub salt: String,               // 32+ hex chars
    pub tag: String,                // 32 hex chars
    pub verified: bool,
    pub created_at: i64,
}
```

### EmailPermission

```rust
pub struct EmailPermission {
    pub service_name: String,
    pub purpose: String,
    pub granted: bool,
    pub granted_at: i64,
    pub revoked_at: Option<i64>,
}
```

---

## Code Quality

### Update Chain Pattern (Reusable)

```rust
fn get_latest_record(original_hash: ActionHash) -> ExternResult<Option<Record>> {
    let mut current_hash = original_hash;
    
    loop {
        let details = get_details(current_hash.clone(), GetOptions::default())?
            .ok_or(wasm_error!("Not found in chain"))?;
        
        match details {
            Details::Record(record_details) => {
                if let Some(latest_update) = record_details.updates.last() {
                    current_hash = latest_update.action_address().clone();
                } else {
                    return Ok(Some(record_details.record));
                }
            }
            _ => return Err(wasm_error!("Expected Record details")),
        }
    }
}
```

**Use this pattern** for any future entry types that support updates!

---

## Next Steps

Before production launch:

1. Create v1.6 (pre-launch version)
2. Test migration v1.5 → v1.6 with fresh accounts
3. Verify all functionality on v1.6
4. Deploy v1.6 to production
5. Monitor for any issues

---

**✅ PRODUCTION READY**  
**Status**: Current version  
**Last Updated**: November 2, 2025

