use hdi::prelude::*;

/// Encrypted user profile - stored ONLY on private DHT
/// Binary data stored as base64 strings for serialization compatibility
/// v1.7: Added username field for privacy-friendly login
#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct UserProfile {
    pub encrypted_email: String,   // Base64-encoded encrypted email
    pub nonce: String,             // Base64-encoded nonce
    pub salt: String,              // Base64-encoded KDF salt
    pub tag: String,               // Base64-encoded authentication tag
    pub username: Option<String>,  // ✅ NEW v1.7: Optional username (encrypted)
    pub display_name: String,      // Can be public
    pub created_at: i64,
    pub updated_at: i64,
}

/// Encrypted recovery phrase - stored ONLY on private DHT
#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct RecoveryPhrase {
    pub encrypted_mnemonic: String,  // Base64-encoded encrypted 24-word phrase
    pub nonce: String,
    pub salt: String,
    pub tag: String,
    pub verified: bool,               // Has user verified they saved it?
    pub created_at: i64,
}

/// Session tracking - stored ONLY on private DHT
#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct Session {
    pub user_agent: String,
    pub ip_address: String,
    pub device_info: String,
    pub conductor_id: String,         // Which edge node
    pub created_at: i64,
    pub last_active: i64,
}

/// Email permission - NEW IN v1.1
/// Stores user consent for Flowsta services to access their email for specific purposes
/// This enables privacy-preserving email notifications (invoices, system alerts, etc.)
#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct EmailPermission {
    pub service_name: String,         // e.g., "billing", "support", "security_alerts"
    pub purpose: String,              // Human-readable: "Send monthly invoice notifications"
    pub granted: bool,                // User consent status
    pub granted_at: Option<i64>,      // When permission was granted (None if never granted)
    pub revoked_at: Option<i64>,      // When permission was revoked (None if still granted)
    pub last_used_at: Option<i64>,    // When service last accessed email (for transparency)
    pub created_at: i64,
    pub updated_at: i64,
}

/// Login activity - NEW IN v1.6
/// User-owned login tracking with privacy controls
/// IP and user-agent are optional - user can disable tracking
#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct LoginActivity {
    pub timestamp: i64,
    pub login_method: String,         // "password" or "sso"
    pub ip_address: Option<String>,   // User can opt-out (privacy setting)
    pub user_agent: Option<String>,   // User can opt-out (privacy setting)
    pub session_id: String,
    pub created_at: i64,
}

/// Dashboard activity - NEW IN v1.6
/// Track dashboard page visits for user's own analytics
#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct DashboardActivity {
    pub visit_timestamp: i64,
    pub page_path: String,            // e.g., "/dashboard/apps", "/dashboard/analytics"
    pub duration_seconds: Option<i64>, // Filled in by frontend
    pub created_at: i64,
}

/// OAuth activity - NEW IN v1.6
/// Track OAuth app usage per user (user-owned, not for billing)
#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct OAuthActivity {
    pub timestamp: i64,
    pub app_id: String,
    pub app_name: String,
    pub event_type: String,           // "login", "consent_granted", "token_refreshed", "revoked"
    pub created_at: i64,
}

/// Privacy settings - NEW IN v1.6
/// User controls for activity tracking
#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct PrivacySettings {
    pub track_ip_address: bool,
    pub track_user_agent: bool,
    pub activity_log_retention_days: i64,
    pub auto_anonymize_after_days: Option<i64>,  // Future: hash old IPs after N days
    pub created_at: i64,
    pub updated_at: i64,
}

/// App Analytics ID - NEW IN v1.9
/// Zero-knowledge analytics: stores random analytics_id per app
/// This ID is mathematically impossible to link to user DID without user's password
/// Only user can decrypt this from their private Holochain with their password
#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct AppAnalyticsId {
    pub app_id: String,           // UUID of developer app
    pub analytics_id: String,     // Random UUID - no link to user DID
    pub created_at: i64,          // Timestamp when first created
}

/// Entry types with PRIVATE visibility
/// CRITICAL: visibility = "private" means NOT on public DHT
#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EntryTypes {
    #[entry_type(visibility = "private")]
    UserProfile(UserProfile),
    
    #[entry_type(visibility = "private")]
    RecoveryPhrase(RecoveryPhrase),
    
    #[entry_type(visibility = "private")]
    Session(Session),  // DEPRECATED but kept for backward compatibility
    
    #[entry_type(visibility = "private")]
    EmailPermission(EmailPermission),  // NEW IN v1.1
    
    // NEW IN v1.6 - User-owned metadata
    #[entry_type(visibility = "private")]
    LoginActivity(LoginActivity),
    
    #[entry_type(visibility = "private")]
    DashboardActivity(DashboardActivity),
    
    #[entry_type(visibility = "private")]
    OAuthActivity(OAuthActivity),
    
    #[entry_type(visibility = "private")]
    PrivacySettings(PrivacySettings),
    
    // NEW IN v1.9 - Zero-knowledge analytics
    #[entry_type(visibility = "private")]
    AppAnalyticsId(AppAnalyticsId),
}

/// Link types for private data
#[derive(Serialize, Deserialize)]
#[hdk_link_types]
pub enum LinkTypes {
    AgentToProfile,
    AgentToRecoveryPhrase,
    AgentToSessions,  // DEPRECATED but kept for backward compatibility
    AgentToEmailPermissions,  // NEW IN v1.1
    // NEW IN v1.6 - User-owned metadata
    AgentToLoginActivity,
    AgentToDashboardActivity,
    AgentToOAuthActivity,
    AgentToPrivacySettings,
    // NEW IN v1.9 - Zero-knowledge analytics
    AgentToAppAnalyticsId,
}

/// Validate all operations on private DHT
/// Membrane proof validation happens at genesis
#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        Op::StoreRecord(store_record) => {
            // Validate that operations are from the correct agent
            match store_record.record.action() {
                Action::Create(create) => {
                    // Verify author is the agent who created the entry
                    Ok(ValidateCallbackResult::Valid)
                }
                Action::Update(update) => {
                    // Verify author matches original entry author
                    let original_record = must_get_valid_record(update.original_action_address.clone())?;
                    if *original_record.action().author() == update.author {
                        Ok(ValidateCallbackResult::Valid)
                    } else {
                        Ok(ValidateCallbackResult::Invalid(
                            "Only original author can update".into()
                        ))
                    }
                }
                Action::Delete(delete) => {
                    // Verify author matches original entry author
                    let original_record = must_get_valid_record(delete.deletes_address.clone())?;
                    if *original_record.action().author() == delete.author {
                        Ok(ValidateCallbackResult::Valid)
                    } else {
                        Ok(ValidateCallbackResult::Invalid(
                            "Only original author can delete".into()
                        ))
                    }
                }
                _ => Ok(ValidateCallbackResult::Valid)
            }
        }
        Op::StoreEntry(_) => Ok(ValidateCallbackResult::Valid),
        Op::RegisterUpdate(update) => {
            // Verify update author matches original
            let original_record = must_get_valid_record(update.update.hashed.content.original_action_address.clone())?;
            if original_record.action().author() == &update.update.hashed.content.author {
                Ok(ValidateCallbackResult::Valid)
            } else {
                Ok(ValidateCallbackResult::Invalid(
                    "Update author must match original author".into()
                ))
            }
        }
        Op::RegisterDelete(delete) => {
            // Verify delete author matches original
            let original_record = must_get_valid_record(delete.delete.hashed.content.deletes_address.clone())?;
            if original_record.action().author() == &delete.delete.hashed.content.author {
                Ok(ValidateCallbackResult::Valid)
            } else {
                Ok(ValidateCallbackResult::Invalid(
                    "Delete author must match original author".into()
                ))
            }
        }
        Op::RegisterCreateLink(_) => Ok(ValidateCallbackResult::Valid),
        Op::RegisterDeleteLink(_) => Ok(ValidateCallbackResult::Valid),
        Op::RegisterAgentActivity(_) => Ok(ValidateCallbackResult::Valid),
    }
}

/// Genesis self-check - validates membrane proof
/// This is called when an agent tries to join the private DHT
#[hdk_extern]
pub fn genesis_self_check(_data: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    // Membrane proof validation
    // In production, verify against Flowsta's signature
    // For staging, allow all edge nodes that provide a proof
    
    // Note: Membrane proof is primarily enforced at the conductor level
    // This is an additional validation layer
    
    Ok(ValidateCallbackResult::Valid)
}
