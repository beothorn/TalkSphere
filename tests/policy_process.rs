//! Peer policy between real node processes: admission of unconfigured peers, quotas and the
//! decisions that depend on them.
#![cfg(target_os = "linux")]
mod support;
use support::*;

// An authenticated peer nobody configured can read the free Board metadata, but a provider that has
// zero trust in it refuses paid work, and the refusal costs nothing.
#[test]
fn unconfigured_peer_reads_metadata_and_is_refused_paid_work_at_zero_trust() {
    let dir = Dir::new("unconfigured");
    let (mut a, mut b) = (Node::init(&dir.0, "a", None), Node::init(&dir.0, "b", None));
    b.start(None);
    a.start(None);
    let contact = b.contact(b.port);
    let fetched = a.ctl(&["fetch", "--contact", &contact]).unwrap();
    assert!(
        !fetched["provided"].as_array().unwrap().is_empty(),
        "{fetched}"
    );
    a.ctl(&["own", "--peer", &b.id, "--amount", "100"]).unwrap();
    a.ctl(&["trust", "--peer", &b.id, "--millionths", "1"])
        .unwrap();
    let object = dir.0.join("object");
    std::fs::write(&object, b"abc").unwrap();
    let refused = a
        .ctl(&[
            "store",
            "--contact",
            &contact,
            "--file",
            object.to_str().unwrap(),
            "--retention-seconds",
            "60",
            "--max-spend",
            "100",
        ])
        .unwrap_err();
    assert!(refused.contains("denied"), "{refused}");
    assert_eq!(number(&balance(&a, &b), "owned"), 100);
    assert_eq!(number(&balance(&b, &a), "issued"), 0);
}

// With a node connection limit of one, the first authenticated peer stays and a second one is
// closed by admission policy before it can do anything; the first keeps working.
#[test]
fn admission_policy_refuses_connections_beyond_the_limit() {
    let dir = Dir::new("admission");
    let (mut a, mut b, mut c) = (
        Node::init(&dir.0, "a", None),
        Node::init(&dir.0, "b", None),
        Node::init(&dir.0, "c", None),
    );
    b.set_policy(r#"{"version":1,"admission":{"connections":1,"perPeerConnections":1}}"#);
    b.start(None);
    a.start(None);
    c.start(None);
    let contact = b.contact(b.port);
    a.ctl(&["fetch", "--contact", &contact]).unwrap();
    assert!(c.ctl(&["fetch", "--contact", &contact]).is_err());
    let status = b.ctl(&["status"]).unwrap();
    assert_eq!(number(&status, "connections"), 1);
    assert_eq!(status["connectedPeers"][0].as_str().unwrap(), a.id);
    // The admitted peer is unaffected by the refusal.
    assert!(a.ctl(&["fetch", "--contact", &contact]).is_ok());
}
