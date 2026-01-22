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
    pub user_profile: Option<UserProfile>,
    pub recovery_phrase: Option<RecoveryPhrase>,
    pub sessions: Vec<Session>,
    pub export_timestamp: i64,
    pub dna_version: String,
}

/// Export all private data for migration to new DNA version
/// This allows users to export their data from v1.0 and import to v1.1
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
    
    // Get all sessions
    let session_records = get_my_sessions(())?;
    let mut sessions = Vec::new();
    for record in session_records {
        if let Some(session) = record.entry().to_app_option::<Session>().ok().flatten() {
            sessions.push(session);
        }
    }
    debug!("📦 [EXPORT] Found {} sessions", sessions.len());
    
    let export_timestamp = sys_time()?.as_micros();
    
    let exported_data = ExportedData {
        user_profile,
        recovery_phrase,
        sessions,
        export_timestamp,
        dna_version: "1.0".to_string(),
    };
    
    debug!("📦 [EXPORT] Export complete");
    Ok(exported_data)
}

/// Import data from an export bundle (used in new DNA version v1.1)
/// This function will be updated in v1.1 to import data from v1.0
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
    
    // Import sessions
    debug!("📥 [IMPORT] Importing {} sessions", data.sessions.len());
    for session in data.sessions {
        store_session(session)?;
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



