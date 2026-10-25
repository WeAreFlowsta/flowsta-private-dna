//! Private DNA v2.1 coordinator — encrypt-before-gossip, on Holochain 0.7.1.
//!
//! Deliberately tiny: the zome stores and returns opaque ciphertext. It
//! cannot filter, search, or index (payloads are sealed) — all querying
//! happens client-side in Vault after decryption. Resist the
//! urge to add plaintext-taking parameters here; that is how v1.11 leaked.

use hdk::prelude::*;
use private_data_integrity::*;
use std::collections::HashMap;

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
    /// More links to the same record, each with its own tag, so a kind of
    /// record can be asked for on its own (`get_all_sealed_at` with that
    /// prefix) while readers of the main link see it as before.
    #[serde(default)]
    pub extra_tags: Vec<serde_bytes::ByteBuf>,
}

/// Store a record and link it from the shared base.
#[hdk_extern]
pub fn create_sealed_at(input: SealedAtInput) -> ExternResult<Record> {
    check_input(&SealedInput { cipher: input.cipher.clone(), nonce: input.nonce.clone() })?;
    let sealed_hash = create_entry(&EntryZomes::IntegrityPrivateData(EntryTypes::Sealed(Sealed {
        cipher: input.cipher,
        nonce: input.nonce,
    })))?;
    create_link(input.base.clone(), sealed_hash.clone(), LinkTypes::AgentToSealed, LinkTag::new(input.tag))?;
    for extra in input.extra_tags {
        create_link(input.base.clone(), sealed_hash.clone(), LinkTypes::AgentToSealed, LinkTag::new(extra.into_vec()))?;
    }
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

// ── Listing without reading again what the caller holds ────────────────
//
// A `get` costs about 0.3 s on a large cell whatever its strategy, so
// `get_all_sealed_at` took minutes on a device holding thousands of
// records, and a Vault that asked again before the answer came stacked
// those reads up inside the conductor. Records never change (an edit is a
// new record), so the Vault keeps what it has read and this lists every
// live link with records only for targets the caller does not hold: this
// device's own from its chain in two queries, the rest by `get`, at most
// `limit` of those per call so no call runs long.

#[derive(Serialize, Deserialize, Debug)]
pub struct ListRefsInput {
    pub base: AgentPubKey,
    /// As in `get_all_sealed_at`: empty = records with an empty tag only.
    #[serde(default, with = "serde_bytes")]
    pub tag_prefix: Vec<u8>,
    /// Records the caller holds already: listed, not sent again.
    #[serde(default)]
    pub known: Vec<ActionHash>,
    /// At most this many records read by `get` in one call (0 = no limit).
    #[serde(default)]
    pub limit: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListedRef {
    pub target: ActionHash,
    pub link: ActionHash,
    #[serde(with = "serde_bytes")]
    pub tag: Vec<u8>,
    /// None when the caller holds it, when it is not on this device (yet),
    /// or when it was left for the limit.
    pub record: Option<Record>,
    /// Not read in this call for the limit: ask again.
    pub left: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListedRefs {
    pub refs: Vec<ListedRef>,
    /// Records were left out for the limit: ask again.
    pub more: bool,
}

/// Every live link from the base, with the records the caller lacks. Reads
/// what this device holds (never the network).
#[hdk_extern]
pub fn list_sealed_at(input: ListRefsInput) -> ExternResult<ListedRefs> {
    let links = get_links(LinkQuery::try_new(input.base, LinkTypes::AgentToSealed)?, GetStrategy::Local)?;
    let known: HashSet<ActionHash> = input.known.into_iter().collect();
    let me = agent_info()?.agent_initial_pubkey;
    let mut listed = Vec::with_capacity(links.len());
    for link in links {
        let tag = link.tag.into_inner();
        let wanted = if input.tag_prefix.is_empty() { tag.is_empty() } else { tag.starts_with(&input.tag_prefix) };
        if !wanted {
            continue;
        }
        let Ok(target) = ActionHash::try_from(link.target) else {
            continue;
        };
        listed.push((target, link.create_link_hash, tag, link.author == me));
    }
    // A record and its link are written in one call, so a link this agent
    // wrote points at a record on this agent's chain.
    let own_targets: Vec<ActionHash> = listed
        .iter()
        .filter(|(target, _, _, mine)| *mine && !known.contains(target))
        .map(|(target, ..)| target.clone())
        .collect();
    let mut own = from_own_chain(&own_targets)?;
    let mut budget = if input.limit == 0 { u32::MAX } else { input.limit };
    let mut more = false;
    let mut refs = Vec::with_capacity(listed.len());
    for (target, link, tag, _) in listed {
        let (record, left) = if known.contains(&target) {
            (None, false)
        } else if let Some(record) = own.remove(&target) {
            (Some(record), false)
        } else if budget > 0 {
            budget -= 1;
            (get(target.clone(), GetOptions::local())?, false)
        } else {
            more = true;
            (None, true)
        };
        refs.push(ListedRef { target, link, tag, record, left });
    }
    Ok(ListedRefs { refs, more })
}

/// Up to this many records are read with `get`; more with the chain
/// queries (one scan costs about six gets on a large cell).
const CHAIN_SCAN_ABOVE: usize = 6;

/// Sequence numbers this close together are fetched as one range.
const RANGE_GAP: u32 = 16;

/// The wanted records that sit on this agent's own chain, by two chain
/// queries instead of one `get` each; records this agent deleted are left
/// out, as `get` leaves them out.
fn from_own_chain(targets: &[ActionHash]) -> ExternResult<HashMap<ActionHash, Record>> {
    if targets.len() <= CHAIN_SCAN_ABOVE {
        return Ok(HashMap::new());
    }
    let wanted: HashSet<&ActionHash> = targets.iter().collect();
    // Actions only (no entry is loaded): where the wanted records sit, and
    // which of them this agent deleted.
    let actions = query(ChainQueryFilter::new().include_entries(false))?;
    let mut seqs = Vec::new();
    let mut deleted = HashSet::new();
    for record in &actions {
        match &record.action().data {
            ActionData::Create(_) if wanted.contains(record.action_address()) => seqs.push(record.action().action_seq()),
            ActionData::Delete(delete) => {
                deleted.insert(delete.deletes_address.clone());
            }
            _ => {}
        }
    }
    drop(actions);
    // Entries only for those sequence numbers. NEVER ask for entries without
    // a sequence range: the conductor loads every entry of the chain before
    // it filters (on a large cell that froze a 16 GB machine).
    let mut out = HashMap::new();
    for (from, to) in seq_ranges(&mut seqs, RANGE_GAP) {
        let records = query(
            ChainQueryFilter::new()
                .sequence_range(ChainQueryFilterRange::ActionSeqRange(from, to))
                .action_type(ActionType::Create)
                .include_entries(true),
        )?;
        for record in records {
            if wanted.contains(record.action_address()) && !deleted.contains(record.action_address()) {
                out.insert(record.action_address().clone(), record);
            }
        }
    }
    Ok(out)
}

/// Sorted, merged (from, to) ranges covering every number, joining numbers
/// no further apart than `gap`.
fn seq_ranges(seqs: &mut Vec<u32>, gap: u32) -> Vec<(u32, u32)> {
    seqs.sort_unstable();
    seqs.dedup();
    let mut out: Vec<(u32, u32)> = Vec::new();
    for &s in seqs.iter() {
        match out.last_mut() {
            Some((_, to)) if s - *to <= gap => *to = s,
            _ => out.push((s, s)),
        }
    }
    out
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
    // 0.7: a grant names its constraint and its capability (a zome call here;
    // direct signals would be a separate `DirectSignal` grant).
    create_cap_grant(CapGrant::new_zome_call_grant(
        PIECE_GRANT_TAG.into(),
        GrantConstraint::Unrestricted,
        GrantedFunctions::Listed(functions),
    ))?;
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
