# DNA v1.1 - EmailPermission Feature

**Released**: October 2024  
**Status**: Legacy (replaced by v1.5)

---

## What's New in v1.1

### New Entry Type: EmailPermission

Added support for user-granted email access permissions, enabling services (like billing) to request access to decrypt the user's email address.

```rust
pub struct EmailPermission {
    pub service_name: String,      // e.g., "billing"
    pub purpose: String,             // Human-readable purpose
    pub granted: bool,               // Permission status
    pub granted_at: i64,
    pub revoked_at: Option<i64>,
}
```

### New Functions

- `grant_email_permission(service_name, purpose)`
- `revoke_email_permission(service_name)`
- `get_email_permissions()` - Returns all permissions
- `get_email_permission(service_name)` - Returns specific permission

---

## Known Issues

⚠️ **Same Update Chain Bug as v1.0**: The `get_user_profile` and `get_recovery_phrase` functions still don't follow the update chain.

⚠️ **New Bug**: `get_email_permissions` also doesn't follow update chains for individual permissions.

**Fixed In**: 
- v1.2/v1.3 (for profiles and email permissions)
- v1.4 (for recovery phrases)

---

## Migration Path

- ✅ v1.0 → v1.1 (data migrated successfully)
- ✅ v1.1 → v1.3 (skipped v1.2)

---

## Network Seed

```yaml
network_seed: "flowsta-private-network-v1.1"
```

**Important**: Changed from v1.0 to ensure separate network!

---

## Build

```bash
cd v1.1
bash build.sh
# Output: workdir/flowsta_private_v1_1_happ.happ
```

---

**Historical Version** - Not recommended for new deployments

