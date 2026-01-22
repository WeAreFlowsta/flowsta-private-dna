# DNA v1.3 - Update Chain Fix for UserProfile

**Released**: October 2024  
**Status**: Legacy (replaced by v1.5)

---

## What's Fixed in v1.3

### ✅ Update Chain Following for `get_user_profile`

Fixed the critical bug where `get_user_profile` didn't follow the update chain, causing password changes to fail after the first update.

```rust
// v1.3 implementation (INCOMPLETE - only follows ONE level!)
pub fn get_user_profile(_: ()) -> ExternResult<Option<Record>> {
    let links = get_links(...)?;
    if let Some(link) = links.first() {
        let hash = ActionHash::try_from(link.target.clone())?;
        let details = get_details(hash, GetOptions::default())?;
        
        match details {
            Some(Details::Record(record_details)) => {
                // Check for updates
                if let Some(updates) = record_details.updates.last() {
                    return get(updates.action_address().clone(), GetOptions::default());
                }
                return Ok(Some(record_details.record));
            }
            _ => Ok(None),
        }
    } else {
        Ok(None)
    }
}
```

**Note**: This STILL had a bug! It only follows ONE level of updates. Fixed properly in v1.5.

---

## Known Issues

⚠️ **Incomplete Fix**: Only follows the LAST update, not the ENTIRE chain. After 2+ password changes, the function returns stale data.

**Symptom**: 
- First password change: ✅ Works
- Second password change: ❌ Fails (old password still works)

⚠️ **Recovery Phrase Still Broken**: `get_recovery_phrase` still doesn't follow update chain at all.

**Fixed In**: v1.5 (recursive loop implementation)

---

## Migration Path

- ✅ v1.1 → v1.3 (successful)
- ✅ v1.3 → v1.4 (adds recovery phrase fix)

---

## Network Seed

```yaml
network_seed: "flowsta-private-network-v1.3"
```

---

## Build

```bash
cd v1.3
bash build.sh
# Output: workdir/flowsta_private_v1_3_happ.happ
```

---

**Historical Version** - Not recommended for new deployments

