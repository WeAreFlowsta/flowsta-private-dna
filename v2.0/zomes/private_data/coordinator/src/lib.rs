//! Private DNA v2.0 coordinator — encrypt-before-gossip.
//!
//! Deliberately tiny: the zome stores and returns opaque ciphertext. It
//! cannot filter, search, or index (payloads are sealed) — all querying
//! happens client-side in Vault after decryption. Resist the
//! urge to add plaintext-taking parameters here; that is how v1.11 leaked.

use hdk::prelude::*;
use private_data_integrity::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityPrivateData(private_data_integrity::EntryTypes),
}

/// XSalsa20 nonce length — the only structural check the zome performs
/// (length is not content).
const NONCE_LEN: usize = 24;

#[derive(Serialize, Deserialize, Debug)]
pub struct SealedInput {
    #[serde(with = "serde_bytes")]
    pub cipher: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
}

fn check_input(input: &SealedInput) -> ExternResult<()> {
    if input.nonce.len() != NONCE_LEN {
        return Err(wasm_error!("nonce must be 24 bytes"));
    }
    // 16-byte Poly1305 MAC + at least a minimal payload
    if input.cipher.len() <= 16 {
        return Err(wasm_error!("cipher too short"));
    }
    Ok(())
}

/// Store a sealed record and link it from the agent.
#[hdk_extern]
pub fn create_sealed(input: SealedInput) -> ExternResult<Record> {
    check_input(&input)?;

    let sealed_hash = create_entry(&EntryZomes::IntegrityPrivateData(
        EntryTypes::Sealed(Sealed {
            cipher: input.cipher,
            nonce: input.nonce,
        }),
    ))?;

    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    create_link(
        my_agent_pub_key,
        sealed_hash.clone(),
        LinkTypes::AgentToSealed,
        (),
    )?;

    get(sealed_hash, GetOptions::default())?
        .ok_or(wasm_error!("Could not find the newly created sealed record"))
}

/// All live sealed records for this agent (ciphertext — the caller
/// decrypts). Deleted records drop out naturally (`get` returns None for
/// tombstoned entries).
#[hdk_extern]
pub fn get_all_sealed(_: ()) -> ExternResult<Vec<Record>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    let links = get_links(
        LinkQuery::try_new(my_agent_pub_key, LinkTypes::AgentToSealed)?,
        GetStrategy::default(),
    )?;

    let mut records = Vec::with_capacity(links.len());
    for link in links {
        let Ok(action_hash) = ActionHash::try_from(link.target.clone()) else {
            continue;
        };
        if let Some(record) = get(action_hash, GetOptions::default())? {
            records.push(record);
        }
    }
    Ok(records)
}

/// Delete a sealed record (tombstone) and clean up its agent link.
/// Validation enforces that only the original author can delete.
#[hdk_extern]
pub fn delete_sealed(sealed_hash: ActionHash) -> ExternResult<ActionHash> {
    let delete_hash = delete_entry(sealed_hash.clone())?;

    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    let links = get_links(
        LinkQuery::try_new(my_agent_pub_key, LinkTypes::AgentToSealed)?,
        GetStrategy::default(),
    )?;
    for link in links {
        if let Ok(target) = ActionHash::try_from(link.target.clone()) {
            if target == sealed_hash {
                delete_link(link.create_link_hash.clone(), GetOptions::default())?;
            }
        }
    }

    Ok(delete_hash)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReplaceSealedInput {
    pub original_hash: ActionHash,
    pub replacement: SealedInput,
}

/// Supersede a sealed record: create the replacement, then tombstone the
/// original (records are otherwise immutable; there is no in-place update).
#[hdk_extern]
pub fn replace_sealed(input: ReplaceSealedInput) -> ExternResult<Record> {
    check_input(&input.replacement)?;
    let record = create_sealed(input.replacement)?;
    delete_sealed(input.original_hash)?;
    Ok(record)
}

/// Full-state export for CAL/backup and migration tooling: every live
/// sealed record, as stored (ciphertext). Identical shape to
/// get_all_sealed — kept as a distinct name so export call-sites read as
/// exports (the v1.11 convention).
#[hdk_extern]
pub fn export_all_data(_: ()) -> ExternResult<Vec<Record>> {
    get_all_sealed(())
}
