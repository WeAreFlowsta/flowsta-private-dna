//! Flowsta Private DNA v2.1 on a real Holochain 0.7.1 conductor.
//!
//! Loads the packed bundle from `../workdir/flowsta_private_v2_1.dna`. Two
//! devices of one person on one per-user network: records written by either
//! are listed from the shared base on both; a device cannot update or delete
//! another device's record (validation) but retires it by its link; the
//! listing that sends only what the caller lacks; the piece lane's grant.
//! Run with `--test-threads=1`.

use holochain::prelude::*;
use holochain::sweettest::*;
use private_data_coordinator::{
    ListAtInput, ListRefsInput, ListedRefs, ListedSealed, Piece, ReplaceAtInput, RetireAtInput, SealedAtInput,
    SealedInput, SendPieceInput,
};
use std::path::Path;

const ZOME: &str = "private_data";

async fn dna() -> DnaFile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../workdir/flowsta_private_v2_1.dna");
    SweetDnaFile::from_bundle(&path)
        .await
        .expect("v2.1/workdir/flowsta_private_v2_1.dna loads (run build.sh first)")
}

fn sealed_at(base: &AgentPubKey, byte: u8, tag: &[u8]) -> SealedAtInput {
    SealedAtInput {
        base: base.clone(),
        cipher: vec![byte; 48],
        nonce: vec![byte; 24],
        tag: tag.to_vec(),
        extra_tags: vec![],
    }
}

/// One device: create, list, replace, retire, delete; the nonce and cipher checks.
#[tokio::test(flavor = "multi_thread")]
async fn one_device() {
    let mut conductor = SweetConductor::standard().await;
    let app = conductor.setup_app("private", [&dna().await]).await.unwrap();
    let zome = app.cells()[0].zome(ZOME);
    let base = app.agent().clone();

    let r: Result<Record, _> = conductor
        .call_fallible(&zome, "create_sealed", SealedInput { cipher: vec![1; 48], nonce: vec![1; 23] })
        .await;
    assert!(r.is_err(), "a 23-byte nonce is refused");
    let r: Result<Record, _> = conductor
        .call_fallible(&zome, "create_sealed", SealedInput { cipher: vec![1; 16], nonce: vec![1; 24] })
        .await;
    assert!(r.is_err(), "a cipher no longer than the MAC is refused");

    let a: Record = conductor.call(&zome, "create_sealed_at", sealed_at(&base, 1, b"")).await;
    let b: Record = conductor.call(&zome, "create_sealed_at", sealed_at(&base, 2, b"marker:x")).await;
    let plain: Vec<ListedSealed> = conductor
        .call(&zome, "get_all_sealed_at", ListAtInput { base: base.clone(), tag_prefix: vec![], network: false })
        .await;
    assert_eq!(plain.len(), 1, "an empty prefix lists records with an empty tag only");
    assert_eq!(plain[0].record.action_address(), a.action_address());
    let markers: Vec<ListedSealed> = conductor
        .call(&zome, "get_all_sealed_at", ListAtInput { base: base.clone(), tag_prefix: b"marker:".to_vec(), network: false })
        .await;
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].record.action_address(), b.action_address());

    // The listing sends only what the caller lacks.
    let refs: ListedRefs = conductor
        .call(&zome, "list_sealed_at", ListRefsInput { base: base.clone(), tag_prefix: vec![], known: vec![a.action_address().clone()], limit: 0 })
        .await;
    assert_eq!(refs.refs.len(), 1);
    assert!(refs.refs[0].record.is_none(), "a known record is listed without its bytes");
    assert!(!refs.more);

    // Replace, then retire.
    let c: Record = conductor
        .call(&zome, "replace_sealed_at", ReplaceAtInput { original: a.action_address().clone(), replacement: sealed_at(&base, 3, b"") })
        .await;
    let plain: Vec<ListedSealed> = conductor
        .call(&zome, "get_all_sealed_at", ListAtInput { base: base.clone(), tag_prefix: vec![], network: false })
        .await;
    assert_eq!(plain.len(), 1);
    assert_eq!(plain[0].record.action_address(), c.action_address());
    let removed: u32 = conductor
        .call(&zome, "retire_sealed_at", RetireAtInput { base: base.clone(), target: c.action_address().clone() })
        .await;
    assert_eq!(removed, 1);
    let plain: Vec<ListedSealed> = conductor
        .call(&zome, "get_all_sealed_at", ListAtInput { base: base.clone(), tag_prefix: vec![], network: false })
        .await;
    assert!(plain.is_empty());

    // The piece lane's grant: created once.
    let first: bool = conductor.call(&zome, "ensure_piece_grant", ()).await;
    let second: bool = conductor.call(&zome, "ensure_piece_grant", ()).await;
    assert!(first && !second);
}

/// Two devices on one network: a record from either is listed from the
/// shared base on both; the other device cannot delete it (validation) but
/// retires it by its link; a piece can be sent to the other device.
#[tokio::test(flavor = "multi_thread")]
async fn two_devices() {
    let dna = dna().await;
    let mut conductors = SweetConductorBatch::standard(2).await;
    let apps = conductors.setup_app("private", [&dna]).await.unwrap();
    conductors.exchange_peer_info().await;
    let ((d1,), (d2,)) = apps.into_tuples();
    let z1 = d1.zome(ZOME);
    let z2 = d2.zome(ZOME);
    let c1 = conductors.get(0).unwrap();
    let c2 = conductors.get(1).unwrap();
    let base = d1.agent_pubkey().clone();

    let r1: Record = c1.call(&z1, "create_sealed_at", sealed_at(&base, 5, b"")).await;
    let r2: Record = c2.call(&z2, "create_sealed_at", sealed_at(&base, 6, b"")).await;
    await_consistency([&d1, &d2]).await.expect("both devices hold both records");
    let on_1: Vec<ListedSealed> = c1
        .call(&z1, "get_all_sealed_at", ListAtInput { base: base.clone(), tag_prefix: vec![], network: true })
        .await;
    assert_eq!(on_1.len(), 2, "device 1 lists the record device 2 wrote");
    let on_2: Vec<ListedSealed> = c2
        .call(&z2, "get_all_sealed_at", ListAtInput { base: base.clone(), tag_prefix: vec![], network: true })
        .await;
    assert_eq!(on_2.len(), 2);

    // Device 2 cannot delete device 1's record: authorship is validated.
    let r: Result<ActionHash, _> = c2.call_fallible(&z2, "delete_sealed", r1.action_address().clone()).await;
    assert!(r.is_err(), "only the author deletes an entry");
    // But it retires it by removing the link from the shared base.
    let removed: u32 = c2
        .call(&z2, "retire_sealed_at", RetireAtInput { base: base.clone(), target: r1.action_address().clone() })
        .await;
    assert_eq!(removed, 1);
    await_consistency([&d1, &d2]).await.expect("the link removal reaches device 1");
    let on_1: Vec<ListedSealed> = c1
        .call(&z1, "get_all_sealed_at", ListAtInput { base: base.clone(), tag_prefix: vec![], network: true })
        .await;
    assert_eq!(on_1.len(), 1);
    assert_eq!(on_1[0].record.action_address(), r2.action_address());

    // The piece lane: device 2 grants, device 1 sends.
    let _: bool = c2.call(&z2, "ensure_piece_grant", ()).await;
    let _: () = c1
        .call(
            &z1,
            "send_piece",
            SendPieceInput { to: d2.agent_pubkey().clone(), piece: Piece { id: "t1".into(), kind: "data".into(), seq: 0, total: 1, data: vec![7; 100] } },
        )
        .await;
}
