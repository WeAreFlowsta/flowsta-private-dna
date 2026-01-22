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
        let profile_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid profile hash"))?;
        return get(profile_hash, GetOptions::default());
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
#[hdk_extern]
pub fn get_recovery_phrase(_: ()) -> ExternResult<Option<Record>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    // Get links from agent to recovery phrase
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToRecoveryPhrase)?
            .build(),
    )?;
    
    // Debug: Log how many links we found
    hdk::prelude::debug!("🔍 [GET] Found {} recovery phrase links", links.len());
    
    // If we have multiple links, this is a bug - but let's handle it
    // Get ALL recovery phrases and return the one with the most recent created_at
    if links.is_empty() {
        hdk::prelude::debug!("🔍 [GET] No links found, returning None");
        return Ok(None);
    }
    
    let mut most_recent: Option<(Record, i64, bool)> = None;
    let mut entry_count = 0;
    
    for (i, link) in links.iter().enumerate() {
        let recovery_phrase_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid recovery phrase hash"))?;
        
        if let Some(record) = get(recovery_phrase_hash, GetOptions::default())? {
            // Try to extract the recovery phrase and check its timestamp
            if let Some(recovery_phrase) = record.entry().to_app_option::<RecoveryPhrase>().ok().flatten() {
                entry_count += 1;
                hdk::prelude::debug!("🔍 [GET] Entry #{}: verified={}, created_at={}", 
                    i, recovery_phrase.verified, recovery_phrase.created_at);
                
                let is_newer = most_recent.is_none() || recovery_phrase.created_at > most_recent.as_ref().unwrap().1;
                
                if is_newer {
                    hdk::prelude::debug!("🔍 [GET] Entry #{} is most recent so far", i);
                    most_recent = Some((record, recovery_phrase.created_at, recovery_phrase.verified));
                } else {
                    hdk::prelude::debug!("🔍 [GET] Entry #{} is older, skipping", i);
                }
            }
        }
    }
    
    if let Some((_, timestamp, verified)) = &most_recent {
        hdk::prelude::debug!("🔍 [GET] Returning: verified={}, created_at={}", verified, timestamp);
    }
    
    Ok(most_recent.map(|(record, _, _)| record))
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


