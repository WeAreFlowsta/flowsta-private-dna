//! Private DNA v2.0 — encrypt-before-gossip.
//!
//! v1.11 had 11 entry types, most carrying plaintext PII, all marked
//! private-visibility (no gossip at all). v2 inverts the model: ONE opaque
//! entry type that DOES gossip — but only ciphertext ever reaches the DHT.
//! Entry type, timestamps, app ids, relationships all live INSIDE the
//! cipher (`SealedPayload`, sealed/unsealed in Vault — the zome performs
//! zero crypto and can perform zero filtering; that is the design).
//!
//! Key: XSalsa20-Poly1305 secretbox with the per-user phrase-derived
//! data key (flowsta-vault `key_derivation::derive_data_encryption_key`).
//! Network: per-user network seed derived from the phrase, supplied at
//! install time — the manifest default is a placeholder that MUST be
//! overridden (a shared network would gossip strangers' ciphertext).
//!
//! RecoveryPhrase and TotpConfig deliberately do NOT exist here: the
//! phrase never enters a cell for device-hosted users, and TOTP moves
//! Vault-local. Root secrets are never gossiped.

use hdi::prelude::*;

/// An opaque encrypted record. The only entry type in v2.
///
/// PUBLIC visibility — it replicates to the user's other devices (and any
/// future designated backup peers). What replicates is ciphertext + a
/// random nonce; Poly1305 authenticates the whole blob.
#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct Sealed {
    #[serde(with = "serde_bytes")]
    pub cipher: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>, // 24 bytes (XSalsa20)
}

/// PUBLIC visibility (the default) — Sealed records gossip; that is the
/// point. Only ciphertext ever leaves the device.
#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EntryTypes {
    Sealed(Sealed),
}

/// One link type, empty tags, base = agent key. A single type means
/// per-link-type counts can't leak activity categories (v1.11 leaked
/// app_id in a link TAG and category volumes via 11 typed links).
#[derive(Serialize, Deserialize)]
#[hdk_link_types]
pub enum LinkTypes {
    AgentToSealed,
}

/// Validation is authorship-only and content-blind (identical rules to
/// v1.11): Create always valid; Update/Delete only by the original author.
/// It MUST stay content-blind — the payload is ciphertext by design.
#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        Op::StoreRecord(store_record) => {
            match store_record.record.action() {
                Action::Create(_) => Ok(ValidateCallbackResult::Valid),
                Action::Update(update) => {
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

/// Genesis self-check. The per-user network seed is the real membrane —
/// only devices holding the user's phrase can derive it.
#[hdk_extern]
pub fn genesis_self_check(_data: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
