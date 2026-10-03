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

// ── Several devices, one identity ──────────────────────────────────────
//
// A person's devices each run their own agent in the per-user network.
// Records are linked from ONE shared base (the identity's agent key), so
// every device lists the same set whichever device wrote a record.
// Entry updates and deletes stay author-only (integrity); a device
// retires another device's record by removing its LINK from the base.

#[derive(Serialize, Deserialize, Debug)]
pub struct SealedAtInput {
    pub base: AgentPubKey,
    #[serde(with = "serde_bytes")]
    pub cipher: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
    /// Empty for sealed records; markers carry a short tag.
    #[serde(default, with = "serde_bytes")]
    pub tag: Vec<u8>,
}

/// Store a record and link it from the shared base.
#[hdk_extern]
pub fn create_sealed_at(input: SealedAtInput) -> ExternResult<Record> {
    check_input(&SealedInput { cipher: input.cipher.clone(), nonce: input.nonce.clone() })?;
    let sealed_hash = create_entry(&EntryZomes::IntegrityPrivateData(EntryTypes::Sealed(Sealed {
        cipher: input.cipher,
        nonce: input.nonce,
    })))?;
    create_link(input.base, sealed_hash.clone(), LinkTypes::AgentToSealed, LinkTag::new(input.tag))?;
    get(sealed_hash, GetOptions::local())?
        .ok_or(wasm_error!("Could not find the newly created sealed record"))
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListAtInput {
    pub base: AgentPubKey,
    /// Links whose tag starts with this are returned; empty = records with an empty tag only.
    #[serde(default, with = "serde_bytes")]
    pub tag_prefix: Vec<u8>,
    /// Ask the network instead of reading what this device holds.
    #[serde(default)]
    pub network: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListedSealed {
    pub record: Record,
    pub link: ActionHash,
    #[serde(with = "serde_bytes")]
    pub tag: Vec<u8>,
}

/// Every live record linked from the base, whichever device wrote it.
#[hdk_extern]
pub fn get_all_sealed_at(input: ListAtInput) -> ExternResult<Vec<ListedSealed>> {
    let (link_strategy, get_options) = if input.network {
        (GetStrategy::Network, GetOptions::network())
    } else {
        (GetStrategy::Local, GetOptions::local())
    };
    let links = get_links(LinkQuery::try_new(input.base, LinkTypes::AgentToSealed)?, link_strategy)?;
    let mut out = Vec::with_capacity(links.len());
    for link in links {
        let tag = link.tag.into_inner();
        let wanted = if input.tag_prefix.is_empty() { tag.is_empty() } else { tag.starts_with(&input.tag_prefix) };
        if !wanted {
            continue;
        }
        let Ok(action_hash) = ActionHash::try_from(link.target.clone()) else {
            continue;
        };
        if let Some(record) = get(action_hash, get_options.clone())? {
            out.push(ListedSealed { record, link: link.create_link_hash, tag });
        }
    }
    Ok(out)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RetireAtInput {
    pub base: AgentPubKey,
    pub target: ActionHash,
}

/// Retire a record: remove its links from the base (any device may), and
/// tombstone the entry when this device wrote it. Returns links removed.
#[hdk_extern]
pub fn retire_sealed_at(input: RetireAtInput) -> ExternResult<u32> {
    let links = get_links(LinkQuery::try_new(input.base, LinkTypes::AgentToSealed)?, GetStrategy::Local)?;
    let mut removed = 0u32;
    for link in links {
        if ActionHash::try_from(link.target.clone()).ok().as_ref() == Some(&input.target) {
            delete_link(link.create_link_hash, GetOptions::local())?;
            removed += 1;
        }
    }
    if let Some(record) = get(input.target.clone(), GetOptions::local())? {
        if *record.action().author() == agent_info()?.agent_initial_pubkey {
            delete_entry(input.target)?;
        }
    }
    Ok(removed)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReplaceAtInput {
    pub original: ActionHash,
    pub replacement: SealedAtInput,
}

/// Supersede a record written by any device.
#[hdk_extern]
pub fn replace_sealed_at(input: ReplaceAtInput) -> ExternResult<Record> {
    let base = input.replacement.base.clone();
    let record = create_sealed_at(input.replacement)?;
    retire_sealed_at(RetireAtInput { base, target: input.original })?;
    Ok(record)
}

// ── Bytes between a person's own devices ───────────────────────────────
//
// Large objects (app backups) do not go into entries. A device sends them
// to another of the person's devices as remote signals, in pieces; the
// receiving Vault reassembles and checks the content hash. Delivery is
// best effort: the Vaults acknowledge and resend.

const PIECE_GRANT_TAG: &str = "pieces";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Piece {
    /// Transfer id chosen by the sender.
    pub id: String,
    /// "data" or "ack"; other kinds are passed through untouched.
    pub kind: String,
    pub seq: u32,
    pub total: u32,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PieceFrom {
    pub from: AgentPubKey,
    pub piece: Piece,
}

/// Let the person's other devices deliver pieces to this one. Idempotent.
#[hdk_extern]
pub fn ensure_piece_grant(_: ()) -> ExternResult<bool> {
    let existing = query(
        ChainQueryFilter::new()
            .entry_type(EntryType::CapGrant)
            .include_entries(true),
    )?;
    for record in existing {
        if let Some(Entry::CapGrant(grant)) = record.entry().as_option() {
            if grant.tag == PIECE_GRANT_TAG {
                return Ok(false);
            }
        }
    }
    let mut functions = HashSet::new();
    functions.insert((zome_info()?.name, FunctionName::from("recv_remote_signal")));
    create_cap_grant(CapGrantEntry {
        tag: PIECE_GRANT_TAG.into(),
        access: CapAccess::Unrestricted,
        functions: GrantedFunctions::Listed(functions),
    })?;
    Ok(true)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendPieceInput {
    pub to: AgentPubKey,
    pub piece: Piece,
}

/// Send one piece to another device of the same person.
#[hdk_extern]
pub fn send_piece(input: SendPieceInput) -> ExternResult<()> {
    send_remote_signal(input.piece, vec![input.to])
}

/// A piece arrived: hand it to this device's Vault.
#[hdk_extern]
pub fn recv_remote_signal(piece: Piece) -> ExternResult<()> {
    let from = call_info()?.provenance;
    emit_signal(PieceFrom { from, piece })
}
