//! Private DNA v2.1 — encrypt-before-gossip, on Holochain 0.7.1.
//!
//! v2.1 is the 0.7 generation of v2.0: the same single Sealed entry type and
//! link, validation written for the 0.7 action model (header + data).
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
/// v1.11 and v2.0): Create always valid; Update/Delete only by the original
/// author. It MUST stay content-blind — the payload is ciphertext by design.
#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.flattened::<EntryTypes, LinkTypes>()? {
        // The record authority's view (StoreRecord in 0.6).
        FlatOp::CreateRecord(record_op) => match record_op {
            OpRecord::UpdateEntry { action, .. } => same_author(&action.data.original_action_address, action.author(), "Only original author can update"),
            OpRecord::DeleteEntry { action, .. } => same_author(&action.data.deletes_address, action.author(), "Only original author can delete"),
            _ => Ok(ValidateCallbackResult::Valid),
        },
        // The entry authority's view (RegisterUpdate / RegisterDelete in 0.6).
        FlatOp::Update(update_op) => match update_op {
            OpUpdate::Entry { action, .. } => same_author(&action.data.original_action_address, action.author(), "Update author must match original author"),
            _ => Ok(ValidateCallbackResult::Valid),
        },
        FlatOp::Delete(OpDelete { action }) => same_author(&action.data.deletes_address, action.author(), "Delete author must match original author"),
        _ => Ok(ValidateCallbackResult::Valid),
    }
}

/// The record at `original` must have been authored by `author`.
fn same_author(original: &ActionHash, author: &AgentPubKey, reason: &str) -> ExternResult<ValidateCallbackResult> {
    let original_record = must_get_valid_record(original.clone())?;
    if original_record.action().author() == author {
        Ok(ValidateCallbackResult::Valid)
    } else {
        Ok(ValidateCallbackResult::Invalid(reason.into()))
    }
}

/// Genesis self-check. The per-user network seed is the real membrane —
/// only devices holding the user's phrase can derive it.
#[hdk_extern]
pub fn genesis_self_check(_data: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
