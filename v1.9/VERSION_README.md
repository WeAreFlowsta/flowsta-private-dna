# DNA v1.6 - User-Owned Metadata Migration

**Released**: November 14, 2025  
**Status**: 🚧 **IN DEVELOPMENT**

---

## What's New in v1.6

### ✅ User-Owned Activity Tracking

**Move behavioral metadata from PostgreSQL to Holochain Private DNA**

v1.6 introduces user-owned activity tracking, giving users complete control over their activity history while maintaining billing accuracy.

### New Entry Types

#### 1. LoginActivity
Track login events with optional IP/user-agent (user-controlled)

```rust
pub struct LoginActivity {
    pub timestamp: i64,
    pub login_method: String,        // "password" or "sso"
    pub ip_address: Option<String>,  // User can opt-out
    pub user_agent: Option<String>,  // User can opt-out
    pub session_id: String,
    pub created_at: i64,
}
```

#### 2. DashboardActivity
Track dashboard page visits

```rust
pub struct DashboardActivity {
    pub visit_timestamp: i64,
    pub page_path: String,           // e.g., "/dashboard/apps"
    pub duration_seconds: Option<i64>,
    pub created_at: i64,
}
```

#### 3. OAuthActivity
Track OAuth app usage per user

```rust
pub struct OAuthActivity {
    pub timestamp: i64,
    pub app_id: String,
    pub app_name: String,
    pub event_type: String,          // "login", "consent_granted", "token_refreshed", "revoked"
    pub created_at: i64,
}
```

#### 4. PrivacySettings
User-controlled tracking preferences

```rust
pub struct PrivacySettings {
    pub track_ip_address: bool,
    pub track_user_agent: bool,
    pub activity_log_retention_days: i64,
    pub auto_anonymize_after_days: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}
```

**Defaults**: IP tracking ON, User-agent tracking ON, 90-day retention

---

## Architecture: "Count Publicly, Track Privately"

### PostgreSQL (Billing/Analytics)
- `monthly_active_users` - MAU counts for billing
- `oauth_audit_log` - Aggregate OAuth events
- Anonymized, fast SQL queries

### Holochain Private DNA (User-Owned)
- Detailed login history
- Dashboard activity
- OAuth app usage
- Privacy preferences

**Key Principle**: Users own their detailed activity data. Flowsta only sees aggregate counts for billing.

---

## Migration Path

### v1.5 → v1.6 (First Private DNA Migration with Export/Import)

**Process**:
1. Export all data from v1.5 (including email_permissions!)
2. Validate exported data
3. Install v1.6 DNA
4. Import data to v1.6
5. Create default privacy settings
6. Verify data integrity
7. Update database version

**What's Preserved**:
- ✅ UserProfile (encrypted email)
- ✅ RecoveryPhrase (encrypted mnemonic)
- ✅ EmailPermissions (all grants/revokes)
- ✅ Sessions (for backward compatibility)

**What's New**:
- ✅ Privacy settings (defaults created)
- ✅ Activity tracking enabled

**Timeline**: ~10 seconds per user (background, non-blocking)

---

## Privacy Features

### User Control
- **Default**: IP + user-agent tracking enabled (for security)
- **Option**: Disable IP tracking (privacy mode)
- **Option**: Disable user-agent tracking
- **Option**: Change retention period (30-365 days)

### Security Benefits
Users can detect unauthorized access:
```
✅ Nov 14, 2025 2:34 PM - Login from Chrome (192.168.1.1)
⚠️ Nov 13, 2025 3:47 AM - Login from Firefox (45.123.67.89) ← Suspicious!
```

Without tracking:
```
❓ Nov 14, 2025 2:34 PM - Login  (Was this me?)
❓ Nov 13, 2025 3:47 AM - Login  (Can't tell!)
```

---

## Export/Import Updates

### Enhanced ExportedData

```rust
pub struct ExportedData {
    // v1.5 data (backward compatibility)
    pub user_profile: Option<UserProfile>,
    pub recovery_phrase: Option<RecoveryPhrase>,
    pub sessions: Vec<Session>,
    pub email_permissions: Vec<EmailPermission>,  // ✅ ADDED (was missing in v1.5!)
    
    // v1.6 data (new)
    pub login_activities: Vec<LoginActivity>,
    pub dashboard_activities: Vec<DashboardActivity>,
    pub oauth_activities: Vec<OAuthActivity>,
    pub privacy_settings: Option<PrivacySettings>,
    
    // Metadata
    pub export_timestamp: i64,
    pub dna_version: String,
}
```

**Critical Fix**: v1.5 export was missing `email_permissions`! v1.6 includes them.

---

## Coordinator Functions

### Privacy Settings
- `create_default_privacy_settings()` - Initialize with defaults
- `get_privacy_settings()` - Retrieve user's preferences
- `update_privacy_settings()` - User can change settings

### Login Activity
- `store_login_activity()` - Record login event
- `get_login_activity(limit, offset)` - Retrieve history (paginated)
- `delete_old_login_activity(days)` - Cleanup

### Dashboard Activity
- `store_dashboard_activity()` - Record page visit
- `get_dashboard_activity(limit, offset)` - Retrieve history
- `delete_old_dashboard_activity(days)` - Cleanup

### OAuth Activity
- `store_oauth_activity()` - Record OAuth event
- `get_oauth_activity(limit, offset)` - Retrieve history
- `get_oauth_activity_by_app(app_id)` - Filter by app
- `delete_old_oauth_activity(days)` - Cleanup

### Convenience
- `get_activity_summary()` - Returns counts and stats
  ```rust
  pub struct ActivitySummary {
      pub total_logins: u32,
      pub logins_last_30_days: u32,
      pub unique_apps_used: u32,
      pub dashboard_visits: u32,
      pub last_login: Option<i64>,
  }
  ```

---

## Network Seed

```yaml
network_seed: "flowsta-private-network-v1.6"
```

**Important**: Different network seed = new DHT = no data overlap with v1.5

---

## Build

```bash
cd v1.6
bash build.sh
# Output: workdir/flowsta_private_v1_6_happ.happ
```

---

## Testing Requirements

**Critical**: This is the FIRST Private DNA migration with export/import!

### Test Scenarios

**Test 1: Fresh Export/Import**
1. Create user on v1.5
2. Add email, recovery phrase, 2-3 permissions
3. Change password once (verify update chain)
4. Export data
5. Install v1.6
6. Import data
7. Verify ALL data present
8. Test updates work on v1.6

**Test 2: Full End-to-End Migration**
1. Create user with realistic data
2. Login (triggers background migration)
3. Poll migration status
4. Wait for completion
5. Verify data intact
6. Test activity tracking

**Test 3: Migration Failure Recovery**
1. Create user with corrupted data
2. Trigger migration
3. Verify fails gracefully
4. User can still login (v1.5 fallback)
5. Verify rollback works

**Test 4: Privacy Settings**
1. Default settings (IP ON)
2. Verify IP tracked
3. Disable IP tracking
4. Verify IP is NULL
5. Re-enable
6. Verify IP tracked again

**DO NOT SKIP MIGRATION TESTING!**

---

## Deployment Strategy

### Conservative Rollout

1. **Staging Only**: 1 week
2. **Test with**: 2-3 real users (including you)
3. **Monitor**: Data loss, failures, performance
4. **Production**: Only after successful validation

### Success Criteria (Go/No-Go)

- [ ] All staging users migrated successfully (0 failures)
- [ ] Average migration time < 10 seconds
- [ ] No data loss incidents
- [ ] MAU counts within 5% of expected
- [ ] Activity tracking functional
- [ ] Privacy settings working
- [ ] Performance metrics within targets

**If ALL criteria met → Proceed to production**  
**If ANY criteria failed → Stay on staging, fix issues**

---

## Known Issues

None yet - in development.

---

## Database Changes

### New Table: user_private_dna_versions

```sql
CREATE TABLE user_private_dna_versions (
  user_id UUID PRIMARY KEY,
  private_dna_version VARCHAR(10) NOT NULL DEFAULT '1.5',
  migrated_at TIMESTAMP,
  migration_status VARCHAR(20) DEFAULT 'pending',
  rollback_available_until TIMESTAMP,
  CHECK (private_dna_version IN ('1.0', '1.1', '1.3', '1.4', '1.5', '1.6'))
);
```

### Deprecated Columns (Keep for Now)

```sql
-- DEPRECATED: Moving to Holochain v1.6
users.last_login_at           -- → LoginActivity
users.login_count             -- → COUNT(LoginActivity)
users.dashboard_visit_count   -- → COUNT(DashboardActivity)
users.last_dashboard_visit    -- → MAX(DashboardActivity.timestamp)
```

**Plan**: Remove after 6 months (all users migrated + grace period)

---

## API Changes

### New Endpoints

- `GET /private/privacy-settings` - Get user's settings
- `PUT /private/privacy-settings` - Update settings
- `GET /private/activity/logins` - Login history
- `GET /private/activity/dashboard` - Dashboard history
- `GET /private/activity/oauth` - OAuth history
- `GET /private/activity/summary` - Activity stats
- `GET /private/migration-status` - Check migration progress

### Updated Endpoints

- `/auth/login` - Now tracks activity in Holochain
- `/auth/register` - Installs v1.6, creates defaults
- `/oauth/exchange` - Tracks SSO login activity
- `/oauth/token` - Tracks OAuth app activity

---

## Frontend Changes

### New Pages

- `/dashboard/activity` - Activity history viewer
- `/dashboard/privacy` - Privacy settings

### New Components

- `MigrationStatus.tsx` - Migration progress banner
- `LoginActivityTable.tsx` - Login history display
- `DashboardActivityTable.tsx` - Page visits display
- `OAuthActivityTable.tsx` - OAuth usage display

### SDK Updates

New methods in `flowsta-auth.ts`:
- `getLoginActivity(limit)`
- `getDashboardActivity(limit)`
- `getOAuthActivity(limit)`
- `getActivitySummary()`
- `getPrivacySettings()`
- `updatePrivacySettings(settings)`

---

## Rollback Procedures

### Scenario 1: Single User Failure
- User stays on v1.5 (no impact)
- Fix issue and retry later

### Scenario 2: Data Loss
- **STOP ALL MIGRATIONS**
- Rollback affected users to v1.5
- Investigate and fix
- Re-test thoroughly

### Scenario 3: Performance Issues
- Pause migrations
- Investigate bottleneck
- Optimize
- Resume with rate limiting

### Emergency Rollback
```javascript
// Fallback all users to v1.5
config.holochain.latestPrivateDnaVersion = '1.5';
```

---

## Performance Targets

- Migration time: < 10 seconds
- Login latency: < 2 seconds
- Activity query: < 500ms
- Holochain CPU: < 50%

---

## Security Considerations

### What's Private
- **Holochain**: Detailed activity (IP, user-agent, timestamps, pages)
- **User's node**: All data, user controls access

### What's Public (PostgreSQL)
- **Aggregate counts**: MAU, app usage counts
- **No personal data**: DID only (pseudonymous)

### Privacy Model
- **Default**: Security-focused (track IP/user-agent for detection)
- **Option**: Privacy-focused (disable tracking)
- **User choice**: Balance security vs privacy

---

## Documentation

See also:
- [DNA Migration Guide](../DNA_MIGRATION_GUIDE.md)

---

## Next Steps

1. ✅ Create v1.6 structure
2. ✅ Update DNA config files
3. 🚧 Add new entry types (in progress)
4. 🚧 Add coordinator functions
5. 🚧 Build DNA
6. 🔜 Test locally
7. 🔜 Deploy to staging
8. 🔜 Test with real users
9. 🔜 Deploy to production

---

**✅ PRODUCTION**  
**Status**: Deployed January 2026  
**Version**: v1.9
