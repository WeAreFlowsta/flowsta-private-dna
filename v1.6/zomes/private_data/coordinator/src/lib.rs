use hdk::prelude::*;
use private_data_integrity::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityPrivateData(private_data_integrity::EntryTypes),
}

/// Store encrypted user profile on private DHT
#[hdk_extern]
pub fn store_user_profile(profile: UserProfile) -> ExternResult<Record> {
    // Create the profile entry
    let profile_hash = create_entry(&EntryZomes::IntegrityPrivateData(
        EntryTypes::UserProfile(profile.clone())
    ))?;
    
    // Link from agent to profile (private link)
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    create_link(
        my_agent_pub_key.clone(),
        profile_hash.clone(),
        LinkTypes::AgentToProfile,
        (),
    )?;
    
    // Return the created record
    let record = get(profile_hash, GetOptions::default())?
        .ok_or(wasm_error!("Could not find the newly created profile"))?;
    
    Ok(record)
}

/// Get the current agent's encrypted profile
/// FIXED in v1.5: Now recursively follows ENTIRE update chain (not just one level)
#[hdk_extern]
pub fn get_user_profile(_: ()) -> ExternResult<Option<Record>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    // Get links from agent to profile
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToProfile)?
            .build(),
    )?;
    
    // Get the first (should only be one) profile
    if let Some(link) = links.first() {
        let mut current_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid profile hash"))?;
        
        // Recursively follow the ENTIRE update chain to get the latest version
        loop {
            let details = get_details(current_hash.clone(), GetOptions::default())?
                .ok_or(wasm_error!("Profile not found in chain"))?;
            
            match details {
                Details::Record(record_details) => {
                    // If there are updates, follow to the next one
                    if let Some(latest_update) = record_details.updates.last() {
                        current_hash = latest_update.action_address().clone();
                        // Continue loop to check if THIS record also has updates
                    } else {
                        // No more updates - this is the latest version
                        return Ok(Some(record_details.record));
                    }
                }
                _ => return Err(wasm_error!("Expected Record details")),
            }
        }
    }
    
    Ok(None)
}

/// Update the current agent's encrypted profile
#[hdk_extern]
pub fn update_user_profile(profile: UserProfile) -> ExternResult<Record> {
    // Get the current profile
    let current_profile_record = get_user_profile(())?
        .ok_or(wasm_error!("No profile found to update"))?;
    
    // Update the entry
    let updated_profile_hash = update_entry(
        current_profile_record.action_address().clone(),
        &EntryZomes::IntegrityPrivateData(EntryTypes::UserProfile(profile)),
    )?;
    
    // Return the updated record
    let record = get(updated_profile_hash, GetOptions::default())?
        .ok_or(wasm_error!("Could not find the updated profile"))?;
    
    Ok(record)
}

/// Store encrypted recovery phrase on private DHT
#[hdk_extern]
pub fn store_recovery_phrase(recovery_phrase: RecoveryPhrase) -> ExternResult<ActionHash> {
    let recovery_phrase_hash = create_entry(&EntryZomes::IntegrityPrivateData(
        EntryTypes::RecoveryPhrase(recovery_phrase)
    ))?;
    
    // Link from agent to recovery phrase
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    create_link(
        my_agent_pub_key.clone(),
        recovery_phrase_hash.clone(),
        LinkTypes::AgentToRecoveryPhrase,
        (),
    )?;
    
    Ok(recovery_phrase_hash)
}

/// Get the current agent's encrypted recovery phrase
/// FIXED in v1.4: Now follows the update chain to get the latest version
/// FIXED in v1.5: Now recursively follows ENTIRE update chain (not just one level)
#[hdk_extern]
pub fn get_recovery_phrase(_: ()) -> ExternResult<Option<Record>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    // Get links from agent to recovery phrase
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToRecoveryPhrase)?
            .build(),
    )?;
    
    // Get the first (should only be one) recovery phrase
    if let Some(link) = links.first() {
        let mut current_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid recovery phrase hash"))?;
        
        // Recursively follow the ENTIRE update chain to get the latest version
        // This is CRITICAL - when update_recovery_phrase is called multiple times  
        // (e.g., during repeated password changes), each creates a new update.
        // We must follow the ENTIRE chain, not just one level.
        loop {
            let details = get_details(current_hash.clone(), GetOptions::default())?
                .ok_or(wasm_error!("Recovery phrase not found in chain"))?;
            
            match details {
                Details::Record(record_details) => {
                    // If there are updates, follow to the next one
                    if let Some(latest_update) = record_details.updates.last() {
                        current_hash = latest_update.action_address().clone();
                        // Continue loop to check if THIS record also has updates
                    } else {
                        // No more updates - this is the latest version
                        return Ok(Some(record_details.record));
                    }
                }
                _ => return Err(wasm_error!("Expected Record details")),
            }
        }
    }
    
    Ok(None)
}

/// Mark recovery phrase as verified
#[hdk_extern]
pub fn mark_recovery_phrase_verified(_: ()) -> ExternResult<ActionHash> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    // Get current recovery phrase
    let current_record = get_recovery_phrase(())?
        .ok_or(wasm_error!("No recovery phrase found"))?;
    
    let old_action_hash = current_record.action_address().clone();
    
    let mut recovery_phrase: RecoveryPhrase = current_record
        .entry()
        .to_app_option()
        .map_err(|e| wasm_error!(e))?
        .ok_or(wasm_error!("Malformed recovery phrase"))?;
    
    // Mark as verified AND update timestamp to NOW
    // CRITICAL: We must update created_at so the new entry has a newer timestamp than the old one!
    let old_timestamp = recovery_phrase.created_at;
    let old_verified = recovery_phrase.verified;
    
    recovery_phrase.verified = true;
    recovery_phrase.created_at = sys_time()?.as_micros();
    
    hdk::prelude::debug!("🔧 [VERIFY] OLD: verified={}, created_at={}", old_verified, old_timestamp);
    hdk::prelude::debug!("🔧 [VERIFY] NEW: verified={}, created_at={}", recovery_phrase.verified, recovery_phrase.created_at);
    
    // Delete ALL old links (in case there are multiple)
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key.clone(), LinkTypes::AgentToRecoveryPhrase)?
            .build(),
    )?;
    
    for link in links {
        delete_link(link.create_link_hash)?;
    }
    
    // Create NEW recovery phrase entry (don't use update_entry - it's unreliable with links)
    let new_hash = create_entry(&EntryZomes::IntegrityPrivateData(
        EntryTypes::RecoveryPhrase(recovery_phrase)
    ))?;
    
    // Create new link pointing to the new entry
    create_link(
        my_agent_pub_key,
        new_hash.clone(),
        LinkTypes::AgentToRecoveryPhrase,
        (),
    )?;
    
    // NOTE: We don't delete the old entry because delete_entry() only marks it as deleted,
    // not removes it, which can cause confusion. Instead, we rely on timestamp sorting in
    // get_recovery_phrase() to always return the most recent entry.
    
    Ok(new_hash)
}

/// Update the current agent's encrypted recovery phrase
/// ADDED in v1.4: This function was missing, causing password changes to fail
#[hdk_extern]
pub fn update_recovery_phrase(recovery_phrase: RecoveryPhrase) -> ExternResult<Record> {
    // Get the current recovery phrase
    let current_record = get_recovery_phrase(())?
        .ok_or(wasm_error!("No recovery phrase found to update"))?;
    
    // Update the entry using Holochain's update mechanism
    // This creates a new entry and adds it to the update chain
    let updated_hash = update_entry(
        current_record.action_address().clone(),
        &EntryZomes::IntegrityPrivateData(EntryTypes::RecoveryPhrase(recovery_phrase)),
    )?;
    
    // Return the updated record
    let record = get(updated_hash, GetOptions::default())?
        .ok_or(wasm_error!("Could not find the updated recovery phrase"))?;
    
    Ok(record)
}

/// Store a session on private DHT
#[hdk_extern]
pub fn store_session(session: Session) -> ExternResult<ActionHash> {
    let session_hash = create_entry(&EntryZomes::IntegrityPrivateData(
        EntryTypes::Session(session)
    ))?;
    
    // Link from agent to session
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    create_link(
        my_agent_pub_key.clone(),
        session_hash.clone(),
        LinkTypes::AgentToSessions,
        (),
    )?;
    
    Ok(session_hash)
}

/// Get all sessions for the current agent
#[hdk_extern]
pub fn get_my_sessions(_: ()) -> ExternResult<Vec<Record>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    // Get links from agent to sessions
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToSessions)?
            .build(),
    )?;
    
    // Get all session records
    let mut sessions = Vec::new();
    for link in links {
        let session_hash = ActionHash::try_from(link.target)
            .map_err(|_| wasm_error!("Invalid session hash"))?;
        
        if let Some(record) = get(session_hash, GetOptions::default())? {
            sessions.push(record);
        }
    }
    
    Ok(sessions)
}

/// Delete a specific session
#[hdk_extern]
pub fn delete_session(session_hash: ActionHash) -> ExternResult<ActionHash> {
    delete_entry(session_hash)
}

// ============================================================================
// DNA MIGRATION SUPPORT - Export/Import Functions (v1.0)
// ============================================================================

/// Exported data bundle for migration
#[derive(Serialize, Deserialize, Debug)]
pub struct ExportedData {
    // v1.5 data (backward compatibility)
    pub user_profile: Option<UserProfile>,
    pub recovery_phrase: Option<RecoveryPhrase>,
    pub sessions: Vec<Session>,
    pub email_permissions: Vec<EmailPermission>,  // ✅ CRITICAL: Was missing in v1.5!
    
    // v1.6 data (new, will be empty on v1.5 export)
    pub login_activities: Vec<LoginActivity>,
    pub dashboard_activities: Vec<DashboardActivity>,
    pub oauth_activities: Vec<OAuthActivity>,
    pub privacy_settings: Option<PrivacySettings>,
    
    // Metadata
    pub export_timestamp: i64,
    pub dna_version: String,
}

/// Export all private data for migration to new DNA version
/// UPDATED FOR v1.6: Now includes email_permissions (was missing in v1.5!)
#[hdk_extern]
pub fn export_all_data(_: ()) -> ExternResult<ExportedData> {
    debug!("📦 [EXPORT] Starting export of all private data");
    
    // Get user profile
    let user_profile = if let Some(record) = get_user_profile(())? {
        debug!("📦 [EXPORT] Found user profile");
        record.entry().to_app_option::<UserProfile>().ok().flatten()
    } else {
        debug!("📦 [EXPORT] No user profile found");
        None
    };
    
    // Get recovery phrase
    let recovery_phrase = if let Some(record) = get_recovery_phrase(())? {
        debug!("📦 [EXPORT] Found recovery phrase");
        record.entry().to_app_option::<RecoveryPhrase>().ok().flatten()
    } else {
        debug!("📦 [EXPORT] No recovery phrase found");
        None
    };
    
    // Get all sessions (deprecated but keep for backward compatibility)
    let session_records = get_my_sessions(())?;
    let mut sessions = Vec::new();
    for record in session_records {
        if let Some(session) = record.entry().to_app_option::<Session>().ok().flatten() {
            sessions.push(session);
        }
    }
    debug!("📦 [EXPORT] Found {} sessions", sessions.len());
    
    // ✅ CRITICAL: Export email permissions (was missing in v1.5!)
    let email_permissions = get_email_permissions(())?;
    debug!("📦 [EXPORT] Found {} email permissions", email_permissions.len());
    
    let export_timestamp = sys_time()?.as_micros();
    
    let exported_data = ExportedData {
        user_profile,
        recovery_phrase,
        sessions,
        email_permissions,
        // v1.6 fields will be empty on v1.5 export
        login_activities: Vec::new(),
        dashboard_activities: Vec::new(),
        oauth_activities: Vec::new(),
        privacy_settings: None,
        export_timestamp,
        dna_version: "1.6".to_string(),
    };
    
    debug!("📦 [EXPORT] Export complete");
    Ok(exported_data)
}

/// Import data from an export bundle
/// UPDATED FOR v1.6: Now handles email_permissions and creates default privacy settings
#[hdk_extern]
pub fn import_data(data: ExportedData) -> ExternResult<()> {
    debug!("📥 [IMPORT] Starting import of exported data from DNA v{}", data.dna_version);
    
    // Import user profile if present
    if let Some(profile) = data.user_profile {
        debug!("📥 [IMPORT] Importing user profile");
        store_user_profile(profile)?;
    }
    
    // Import recovery phrase if present
    if let Some(recovery_phrase) = data.recovery_phrase {
        debug!("📥 [IMPORT] Importing recovery phrase");
        store_recovery_phrase(recovery_phrase)?;
    }
    
    // Import sessions (deprecated but keep for backward compatibility)
    debug!("📥 [IMPORT] Importing {} sessions", data.sessions.len());
    for session in data.sessions {
        store_session(session)?;
    }
    
    // ✅ CRITICAL: Import email permissions
    debug!("📥 [IMPORT] Importing {} email permissions", data.email_permissions.len());
    for permission in data.email_permissions {
        // Recreate permission with proper linking
        let permission_hash = create_entry(&EntryZomes::IntegrityPrivateData(
            EntryTypes::EmailPermission(permission.clone())
        ))?;
        
        let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
        create_link(
            my_agent_pub_key,
            permission_hash,
            LinkTypes::AgentToEmailPermissions,
            (),
        )?;
    }
    
    // Import v1.6 activity data (if present - will be empty on v1.5 import)
    debug!("📥 [IMPORT] Importing {} login activities", data.login_activities.len());
    for activity in data.login_activities {
        store_login_activity(activity)?;
    }
    
    debug!("📥 [IMPORT] Importing {} dashboard activities", data.dashboard_activities.len());
    for activity in data.dashboard_activities {
        store_dashboard_activity(activity)?;
    }
    
    debug!("📥 [IMPORT] Importing {} OAuth activities", data.oauth_activities.len());
    for activity in data.oauth_activities {
        store_oauth_activity(activity)?;
    }
    
    // Create default privacy settings if not present in export (v1.5 → v1.6 migration)
    if data.privacy_settings.is_none() {
        debug!("📥 [IMPORT] No privacy settings in export, creating defaults for v1.6");
        create_default_privacy_settings(())?;
    } else if let Some(settings) = data.privacy_settings {
        debug!("📥 [IMPORT] Importing privacy settings");
        let settings_hash = create_entry(&EntryZomes::IntegrityPrivateData(
            EntryTypes::PrivacySettings(settings)
        ))?;
        
        let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
        create_link(
            my_agent_pub_key,
            settings_hash,
            LinkTypes::AgentToPrivacySettings,
            (),
        )?;
    }
    
    debug!("📥 [IMPORT] Import complete");
    Ok(())
}

// ============================================================================
// EMAIL PERMISSIONS - NEW IN v1.1
// ============================================================================

/// Input for granting email permission
#[derive(Serialize, Deserialize, Debug)]
pub struct GrantPermissionInput {
    pub service_name: String,
    pub purpose: String,
}

/// Grant or update email permission for a service
#[hdk_extern]
pub fn grant_email_permission(input: GrantPermissionInput) -> ExternResult<ActionHash> {
    let service_name = input.service_name;
    let purpose = input.purpose;
    debug!("🔐 [PERMISSION] Granting email permission for service: {}", service_name);
    
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    let now = sys_time()?.as_micros();
    
    // Check if permission already exists
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key.clone(), LinkTypes::AgentToEmailPermissions)?
            .build(),
    )?;
    
    // Look for existing permission for this service
    for link in links {
        let permission_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid permission hash"))?;
        
        if let Some(record) = get(permission_hash.clone(), GetOptions::default())? {
            if let Some(mut permission) = record.entry().to_app_option::<EmailPermission>().ok().flatten() {
                if permission.service_name == service_name {
                    // Update existing permission
                    debug!("🔐 [PERMISSION] Updating existing permission");
                    permission.granted = true;
                    permission.granted_at = Some(now);
                    permission.revoked_at = None;
                    permission.updated_at = now;
                    
                    let updated_hash = update_entry(
                        record.action_address().clone(),
                        &EntryZomes::IntegrityPrivateData(EntryTypes::EmailPermission(permission)),
                    )?;
                    
                    return Ok(updated_hash);
                }
            }
        }
    }
    
    // Create new permission
    debug!("🔐 [PERMISSION] Creating new permission");
    let permission = EmailPermission {
        service_name,
        purpose,
        granted: true,
        granted_at: Some(now),
        revoked_at: None,
        last_used_at: None,
        created_at: now,
        updated_at: now,
    };
    
    let permission_hash = create_entry(&EntryZomes::IntegrityPrivateData(
        EntryTypes::EmailPermission(permission)
    ))?;
    
    create_link(
        my_agent_pub_key,
        permission_hash.clone(),
        LinkTypes::AgentToEmailPermissions,
        (),
    )?;
    
    Ok(permission_hash)
}

/// Revoke email permission for a service
#[hdk_extern]
pub fn revoke_email_permission(service_name: String) -> ExternResult<ActionHash> {
    debug!("🔐 [PERMISSION] Revoking email permission for service: {}", service_name);
    
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    let now = sys_time()?.as_micros();
    
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToEmailPermissions)?
            .build(),
    )?;
    
    for link in links {
        let permission_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid permission hash"))?;
        
        if let Some(record) = get(permission_hash, GetOptions::default())? {
            if let Some(mut permission) = record.entry().to_app_option::<EmailPermission>().ok().flatten() {
                if permission.service_name == service_name && permission.granted {
                    // Revoke permission
                    debug!("🔐 [PERMISSION] Found and revoking permission");
                    permission.granted = false;
                    permission.revoked_at = Some(now);
                    permission.updated_at = now;
                    
                    let updated_hash = update_entry(
                        record.action_address().clone(),
                        &EntryZomes::IntegrityPrivateData(EntryTypes::EmailPermission(permission)),
                    )?;
                    
                    return Ok(updated_hash);
                }
            }
        }
    }
    
    Err(wasm_error!("Permission not found or already revoked"))
}

/// Get all email permissions
#[hdk_extern]
pub fn get_email_permissions(_: ()) -> ExternResult<Vec<EmailPermission>> {
    debug!("🔐 [PERMISSION] Getting all email permissions");
    
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToEmailPermissions)?
            .build(),
    )?;
    
    let mut permissions = Vec::new();
    for link in links {
        let permission_hash = ActionHash::try_from(link.target)
            .map_err(|_| wasm_error!("Invalid permission hash"))?;
        
        // Use get_details to follow the update chain and get the latest version
        if let Some(details) = get_details(permission_hash, GetOptions::default())? {
            if let Details::Record(record_details) = details {
                // Follow updates to get the latest version
                let latest_record = if !record_details.updates.is_empty() {
                    // Get the most recent update
                    let latest_hash = record_details.updates[record_details.updates.len() - 1].action_address();
                    get(latest_hash.clone(), GetOptions::default())?
                        .unwrap_or(record_details.record)
                } else {
                    record_details.record
                };
                
                if let Some(permission) = latest_record.entry().to_app_option::<EmailPermission>().ok().flatten() {
                    permissions.push(permission);
                }
            }
        }
    }
    
    debug!("🔐 [PERMISSION] Found {} permissions", permissions.len());
    Ok(permissions)
}

/// Check if a specific service has permission
#[hdk_extern]
pub fn check_email_permission(service_name: String) -> ExternResult<bool> {
    let permissions = get_email_permissions(())?;
    
    for permission in permissions {
        if permission.service_name == service_name && permission.granted {
            debug!("🔐 [PERMISSION] Service '{}' has permission", service_name);
            return Ok(true);
        }
    }
    
    debug!("🔐 [PERMISSION] Service '{}' does NOT have permission", service_name);
    Ok(false)
}

/// Record that a service used the email permission (for transparency)
#[hdk_extern]
pub fn record_permission_usage(service_name: String) -> ExternResult<ActionHash> {
    debug!("🔐 [PERMISSION] Recording usage for service: {}", service_name);
    
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    let now = sys_time()?.as_micros();
    
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToEmailPermissions)?
            .build(),
    )?;
    
    for link in links {
        let permission_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid permission hash"))?;
        
        if let Some(record) = get(permission_hash, GetOptions::default())? {
            if let Some(mut permission) = record.entry().to_app_option::<EmailPermission>().ok().flatten() {
                if permission.service_name == service_name && permission.granted {
                    // Record usage
                    debug!("🔐 [PERMISSION] Recording last_used_at");
                    permission.last_used_at = Some(now);
                    permission.updated_at = now;
                    
                    let updated_hash = update_entry(
                        record.action_address().clone(),
                        &EntryZomes::IntegrityPrivateData(EntryTypes::EmailPermission(permission)),
                    )?;
                    
                    return Ok(updated_hash);
                }
            }
        }
    }
    
    Err(wasm_error!("Permission not found or not granted"))
}

// ============================================================================
// PRIVACY SETTINGS - NEW IN v1.6
// ============================================================================

/// Create default privacy settings for new users or v1.5 → v1.6 migration
#[hdk_extern]
pub fn create_default_privacy_settings(_: ()) -> ExternResult<ActionHash> {
    debug!("🔐 [PRIVACY] Creating default privacy settings");
    
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    // Check if privacy settings already exist
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key.clone(), LinkTypes::AgentToPrivacySettings)?
            .build(),
    )?;
    
    if !links.is_empty() {
        debug!("🔐 [PRIVACY] Privacy settings already exist, skipping");
        return Err(wasm_error!("Privacy settings already exist"));
    }
    
    let now = sys_time()?.as_micros();
    
    // Default settings: security-focused (track IP + user-agent for unauthorized access detection)
    let settings = PrivacySettings {
        track_ip_address: true,            // ON by default for security
        track_user_agent: true,            // ON by default for device identification
        activity_log_retention_days: 90,   // 90-day retention (balance security + privacy)
        auto_anonymize_after_days: None,   // Future feature
        created_at: now,
        updated_at: now,
    };
    
    let settings_hash = create_entry(&EntryZomes::IntegrityPrivateData(
        EntryTypes::PrivacySettings(settings)
    ))?;
    
    create_link(
        my_agent_pub_key,
        settings_hash.clone(),
        LinkTypes::AgentToPrivacySettings,
        (),
    )?;
    
    debug!("🔐 [PRIVACY] Default privacy settings created");
    Ok(settings_hash)
}

/// Get privacy settings (follows update chain)
#[hdk_extern]
pub fn get_privacy_settings(_: ()) -> ExternResult<Option<Record>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToPrivacySettings)?
            .build(),
    )?;
    
    if let Some(link) = links.first() {
        let mut current_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid privacy settings hash"))?;
        
        // Follow update chain to get latest settings
        loop {
            let details = get_details(current_hash.clone(), GetOptions::default())?
                .ok_or(wasm_error!("Privacy settings not found in chain"))?;
            
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
    
    Ok(None)
}

/// Update privacy settings
#[hdk_extern]
pub fn update_privacy_settings(settings: PrivacySettings) -> ExternResult<Record> {
    debug!("🔐 [PRIVACY] Updating privacy settings");
    
    let current_record = get_privacy_settings(())?
        .ok_or(wasm_error!("No privacy settings found to update"))?;
    
    let updated_hash = update_entry(
        current_record.action_address().clone(),
        &EntryZomes::IntegrityPrivateData(EntryTypes::PrivacySettings(settings)),
    )?;
    
    let record = get(updated_hash, GetOptions::default())?
        .ok_or(wasm_error!("Could not find the updated privacy settings"))?;
    
    debug!("🔐 [PRIVACY] Privacy settings updated");
    Ok(record)
}

// ============================================================================
// LOGIN ACTIVITY - NEW IN v1.6
// ============================================================================

/// Store login activity
#[hdk_extern]
pub fn store_login_activity(activity: LoginActivity) -> ExternResult<ActionHash> {
    let activity_hash = create_entry(&EntryZomes::IntegrityPrivateData(
        EntryTypes::LoginActivity(activity)
    ))?;
    
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    create_link(
        my_agent_pub_key,
        activity_hash.clone(),
        LinkTypes::AgentToLoginActivity,
        (),
    )?;
    
    Ok(activity_hash)
}

/// Input for paginated activity queries
#[derive(Serialize, Deserialize, Debug)]
pub struct GetActivityInput {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Get login activity history (paginated, newest first)
#[hdk_extern]
pub fn get_login_activity(input: GetActivityInput) -> ExternResult<Vec<LoginActivity>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToLoginActivity)?
            .build(),
    )?;
    
    let limit = input.limit.unwrap_or(100) as usize;
    let offset = input.offset.unwrap_or(0) as usize;
    
    let mut activities = Vec::new();
    
    // Reverse order (newest first) and apply pagination
    for link in links.iter().rev().skip(offset).take(limit) {
        let hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid activity hash"))?;
        
        if let Some(record) = get(hash, GetOptions::default())? {
            if let Some(activity) = record.entry().to_app_option::<LoginActivity>().ok().flatten() {
                activities.push(activity);
            }
        }
    }
    
    Ok(activities)
}

/// Delete old login activity (cleanup function)
#[hdk_extern]
pub fn delete_old_login_activity(older_than_days: i64) -> ExternResult<u32> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    let now = sys_time()?.as_micros();
    let cutoff = now - (older_than_days * 24 * 60 * 60 * 1_000_000);
    
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToLoginActivity)?
            .build(),
    )?;
    
    let mut deleted_count = 0;
    
    for link in links {
        let hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid activity hash"))?;
        
        if let Some(record) = get(hash.clone(), GetOptions::default())? {
            if let Some(activity) = record.entry().to_app_option::<LoginActivity>().ok().flatten() {
                if activity.created_at < cutoff {
                    delete_entry(hash)?;
                    deleted_count += 1;
                }
            }
        }
    }
    
    debug!("🧹 [CLEANUP] Deleted {} old login activities", deleted_count);
    Ok(deleted_count)
}

// ============================================================================
// DASHBOARD ACTIVITY - NEW IN v1.6
// ============================================================================

/// Store dashboard activity
#[hdk_extern]
pub fn store_dashboard_activity(activity: DashboardActivity) -> ExternResult<ActionHash> {
    let activity_hash = create_entry(&EntryZomes::IntegrityPrivateData(
        EntryTypes::DashboardActivity(activity)
    ))?;
    
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    create_link(
        my_agent_pub_key,
        activity_hash.clone(),
        LinkTypes::AgentToDashboardActivity,
        (),
    )?;
    
    Ok(activity_hash)
}

/// Get dashboard activity history (paginated, newest first)
#[hdk_extern]
pub fn get_dashboard_activity(input: GetActivityInput) -> ExternResult<Vec<DashboardActivity>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToDashboardActivity)?
            .build(),
    )?;
    
    let limit = input.limit.unwrap_or(100) as usize;
    let offset = input.offset.unwrap_or(0) as usize;
    
    let mut activities = Vec::new();
    
    for link in links.iter().rev().skip(offset).take(limit) {
        let hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid activity hash"))?;
        
        if let Some(record) = get(hash, GetOptions::default())? {
            if let Some(activity) = record.entry().to_app_option::<DashboardActivity>().ok().flatten() {
                activities.push(activity);
            }
        }
    }
    
    Ok(activities)
}

/// Delete old dashboard activity
#[hdk_extern]
pub fn delete_old_dashboard_activity(older_than_days: i64) -> ExternResult<u32> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    let now = sys_time()?.as_micros();
    let cutoff = now - (older_than_days * 24 * 60 * 60 * 1_000_000);
    
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToDashboardActivity)?
            .build(),
    )?;
    
    let mut deleted_count = 0;
    
    for link in links {
        let hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid activity hash"))?;
        
        if let Some(record) = get(hash.clone(), GetOptions::default())? {
            if let Some(activity) = record.entry().to_app_option::<DashboardActivity>().ok().flatten() {
                if activity.created_at < cutoff {
                    delete_entry(hash)?;
                    deleted_count += 1;
                }
            }
        }
    }
    
    debug!("🧹 [CLEANUP] Deleted {} old dashboard activities", deleted_count);
    Ok(deleted_count)
}

// ============================================================================
// OAUTH ACTIVITY - NEW IN v1.6
// ============================================================================

/// Store OAuth activity
#[hdk_extern]
pub fn store_oauth_activity(activity: OAuthActivity) -> ExternResult<ActionHash> {
    let activity_hash = create_entry(&EntryZomes::IntegrityPrivateData(
        EntryTypes::OAuthActivity(activity)
    ))?;
    
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    create_link(
        my_agent_pub_key,
        activity_hash.clone(),
        LinkTypes::AgentToOAuthActivity,
        (),
    )?;
    
    Ok(activity_hash)
}

/// Get OAuth activity history (paginated, newest first)
#[hdk_extern]
pub fn get_oauth_activity(input: GetActivityInput) -> ExternResult<Vec<OAuthActivity>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToOAuthActivity)?
            .build(),
    )?;
    
    let limit = input.limit.unwrap_or(100) as usize;
    let offset = input.offset.unwrap_or(0) as usize;
    
    let mut activities = Vec::new();
    
    for link in links.iter().rev().skip(offset).take(limit) {
        let hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid activity hash"))?;
        
        if let Some(record) = get(hash, GetOptions::default())? {
            if let Some(activity) = record.entry().to_app_option::<OAuthActivity>().ok().flatten() {
                activities.push(activity);
            }
        }
    }
    
    Ok(activities)
}

/// Input for app-specific OAuth activity query
#[derive(Serialize, Deserialize, Debug)]
pub struct GetOAuthActivityByAppInput {
    pub app_id: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Get OAuth activity for a specific app
#[hdk_extern]
pub fn get_oauth_activity_by_app(input: GetOAuthActivityByAppInput) -> ExternResult<Vec<OAuthActivity>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToOAuthActivity)?
            .build(),
    )?;
    
    let limit = input.limit.unwrap_or(100) as usize;
    let offset = input.offset.unwrap_or(0) as usize;
    
    let mut activities = Vec::new();
    
    for link in links.iter().rev() {
        let hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid activity hash"))?;
        
        if let Some(record) = get(hash, GetOptions::default())? {
            if let Some(activity) = record.entry().to_app_option::<OAuthActivity>().ok().flatten() {
                if activity.app_id == input.app_id {
                    activities.push(activity);
                }
            }
        }
    }
    
    // Apply pagination after filtering
    let activities: Vec<OAuthActivity> = activities.into_iter()
        .skip(offset)
        .take(limit)
        .collect();
    
    Ok(activities)
}

/// Delete old OAuth activity
#[hdk_extern]
pub fn delete_old_oauth_activity(older_than_days: i64) -> ExternResult<u32> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    let now = sys_time()?.as_micros();
    let cutoff = now - (older_than_days * 24 * 60 * 60 * 1_000_000);
    
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToOAuthActivity)?
            .build(),
    )?;
    
    let mut deleted_count = 0;
    
    for link in links {
        let hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid activity hash"))?;
        
        if let Some(record) = get(hash.clone(), GetOptions::default())? {
            if let Some(activity) = record.entry().to_app_option::<OAuthActivity>().ok().flatten() {
                if activity.created_at < cutoff {
                    delete_entry(hash)?;
                    deleted_count += 1;
                }
            }
        }
    }
    
    debug!("🧹 [CLEANUP] Deleted {} old OAuth activities", deleted_count);
    Ok(deleted_count)
}

// ============================================================================
// ACTIVITY SUMMARY - CONVENIENCE FUNCTION
// ============================================================================

/// Activity summary (for dashboard display)
#[derive(Serialize, Deserialize, Debug)]
pub struct ActivitySummary {
    pub total_logins: u32,
    pub logins_last_30_days: u32,
    pub unique_apps_used: u32,
    pub dashboard_visits: u32,
    pub last_login: Option<i64>,
}

/// Get activity summary (counts and stats)
#[hdk_extern]
pub fn get_activity_summary(_: ()) -> ExternResult<ActivitySummary> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    let now = sys_time()?.as_micros();
    let thirty_days_ago = now - (30 * 24 * 60 * 60 * 1_000_000);
    
    // Count total logins
    let login_links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key.clone(), LinkTypes::AgentToLoginActivity)?
            .build(),
    )?;
    
    let mut total_logins = 0;
    let mut logins_last_30_days = 0;
    let mut last_login: Option<i64> = None;
    
    for link in login_links {
        let hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid hash"))?;
        
        if let Some(record) = get(hash, GetOptions::default())? {
            if let Some(activity) = record.entry().to_app_option::<LoginActivity>().ok().flatten() {
                total_logins += 1;
                
                if activity.timestamp >= thirty_days_ago {
                    logins_last_30_days += 1;
                }
                
                if last_login.is_none() || activity.timestamp > last_login.unwrap() {
                    last_login = Some(activity.timestamp);
                }
            }
        }
    }
    
    // Count dashboard visits
    let dashboard_links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key.clone(), LinkTypes::AgentToDashboardActivity)?
            .build(),
    )?;
    let dashboard_visits = dashboard_links.len() as u32;
    
    // Count unique OAuth apps
    let oauth_links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToOAuthActivity)?
            .build(),
    )?;
    
    let mut app_ids = std::collections::HashSet::new();
    for link in oauth_links {
        let hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid hash"))?;
        
        if let Some(record) = get(hash, GetOptions::default())? {
            if let Some(activity) = record.entry().to_app_option::<OAuthActivity>().ok().flatten() {
                app_ids.insert(activity.app_id);
            }
        }
    }
    let unique_apps_used = app_ids.len() as u32;
    
    Ok(ActivitySummary {
        total_logins,
        logins_last_30_days,
        unique_apps_used,
        dashboard_visits,
        last_login,
    })
}



