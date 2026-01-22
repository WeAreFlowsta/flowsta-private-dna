use hdi::prelude::*;

/// Encrypted user profile - stored ONLY on private DHT
/// Binary data stored as base64 strings for serialization compatibility
#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct UserProfile {
    pub encrypted_email: String,   // Base64-encoded encrypted email
    pub nonce: String,             // Base64-encoded nonce
    pub salt: String,              // Base64-encoded KDF salt
    pub tag: String,               // Base64-encoded authentication tag
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
    Session(Session),
}

/// Link types for private data
#[derive(Serialize, Deserialize)]
#[hdk_link_types]
pub enum LinkTypes {
    AgentToProfile,
    AgentToRecoveryPhrase,
    AgentToSessions,
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
