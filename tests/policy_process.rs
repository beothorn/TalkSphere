//! Peer policy between real node processes: admission of unconfigured peers, quotas and the
//! decisions that depend on them.
#![cfg(target_os = "linux")]
mod support;
use std::time::Duration;
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

// A peer that exceeds the per-identity request rate has its connection closed by quota policy, while a
// peer within the rate is unaffected, and the provider charges nothing for the refused requests.
#[test]
fn request_rate_limit_closes_only_the_noisy_peer() {
    let dir = Dir::new("rate");
    let (mut a, mut b, mut c) = (
        Node::init(&dir.0, "a", None),
        Node::init(&dir.0, "b", None),
        Node::init(&dir.0, "c", None),
    );
    b.set_policy(r#"{"version":1,"quota":{"requestsPerMinute":4}}"#);
    b.start(None);
    a.start(None);
    c.start(None);
    let contact = b.contact(b.port);
    // Each fetch opens a few request streams; the limit allows only a handful per minute.
    let mut outcomes = Vec::new();
    for _ in 0..8 {
        outcomes.push(a.ctl(&["fetch", "--contact", &contact]).is_ok());
    }
    assert!(
        outcomes[0],
        "the first fetch is within the limit: {outcomes:?}"
    );
    assert!(
        outcomes.iter().any(|ok| !ok),
        "the noisy peer was never limited: {outcomes:?}"
    );
    // Another identity has its own allowance.
    assert!(c.ctl(&["fetch", "--contact", &contact]).is_ok());
    assert_eq!(number(&balance(&b, &a), "issued"), 0);
}

// Both peers initiate requests on one connection: a client that only connects outward (the server has
// no address for it) is bought from over the connection the client opened, with exact accounting, and
// once the client disconnects the server fails promptly instead of waiting or guessing an address.
#[test]
fn server_calls_an_outbound_only_client_over_its_own_connection() {
    let dir = Dir::new("reverse");
    let (mut server, mut client) = (
        Node::init(&dir.0, "server", None),
        Node::init(&dir.0, "client", None),
    );
    server.start(None);
    client.start(None);
    // The client connects outward; the server learns of it only through that connection.
    let server_contact = server.contact(server.port);
    client
        .ctl(&["fetch", "--contact", &server_contact])
        .unwrap();
    assert_eq!(
        server.ctl(&["status"]).unwrap()["connectedPeers"][0]
            .as_str()
            .unwrap(),
        client.id
    );
    // The client sells storage to the server; the server pays in the client's credits.
    fund(&client, &server, 100);
    let object = dir.0.join("object");
    std::fs::write(&object, b"abc").unwrap();
    let id_only = client.id.clone();
    let stored = server
        .ctl(&[
            "store",
            "--contact",
            &id_only,
            "--file",
            object.to_str().unwrap(),
            "--retention-seconds",
            "60",
            "--max-spend",
            "100",
        ])
        .unwrap();
    let price = number(&stored, "price");
    assert!(price > 0);
    assert_eq!(number(&balance(&server, &client), "owned"), 100 - price);
    // The issuer sees the same remaining credits the holder owns.
    assert_eq!(number(&balance(&client, &server), "issued"), 100 - price);
    // The client disappears; the server reports an unavailable peer promptly.
    client.stop();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while number(&server.ctl(&["status"]).unwrap(), "connections") != 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "the closed connection was never forgotten"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let started = std::time::Instant::now();
    let error = server
        .ctl(&[
            "store",
            "--contact",
            &id_only,
            "--file",
            object.to_str().unwrap(),
            "--retention-seconds",
            "60",
            "--max-spend",
            "100",
        ])
        .unwrap_err();
    assert!(error.contains("no live connection"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(number(&balance(&server, &client), "owned"), 100 - price);
}

// Wait until `check` holds, failing the test with `message` after the timeout.
fn eventually(message: &str, timeout: Duration, mut check: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + timeout;
    while !check() {
        assert!(std::time::Instant::now() < deadline, "{message}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

// A client started with a server contact reconnects by itself after the server crashes and restarts on
// the same port, and the server sees the client again without any command to either node.
#[test]
fn a_client_reconnects_after_the_server_restarts() {
    let dir = Dir::new("reconnect");
    let (mut server, mut client) = (
        Node::init(&dir.0, "server", None),
        Node::init(&dir.0, "client", None),
    );
    let port = free_port();
    server.start(Some(port));
    client.start_args(None, &["--contact", &server.contact(port)]);
    let sees_client = |server: &Node| {
        server
            .ctl(&["status"])
            .is_ok_and(|s| s["connectedPeers"][0].as_str() == Some(client.id.as_str()))
    };
    eventually(
        "the client never connected",
        Duration::from_secs(20),
        || sees_client(&server),
    );
    server.crash();
    eventually(
        "the client never noticed the loss",
        Duration::from_secs(20),
        || number(&client.ctl(&["status"]).unwrap(), "connections") == 0,
    );
    server.start(Some(port));
    eventually(
        "the client never reconnected",
        Duration::from_secs(30),
        || sees_client(&server),
    );
}

// A server that stops answering without closing its socket is detected by the client's pings: the
// connection is closed after the ping timeout, and the client reconnects once the server answers again.
#[test]
fn an_unanswered_ping_closes_the_connection_and_the_client_reconnects() {
    let dir = Dir::new("ping");
    let (mut server, mut client) = (
        Node::init(&dir.0, "server", None),
        Node::init(&dir.0, "client", None),
    );
    client.set_policy(
        r#"{"version":1,"connection":{"pingSeconds":1,"pingTimeoutSeconds":1,"reconnectMaxSeconds":2}}"#,
    );
    let port = free_port();
    server.start(Some(port));
    client.start_args(None, &["--contact", &server.contact(port)]);
    eventually(
        "the client never connected",
        Duration::from_secs(20),
        || number(&client.ctl(&["status"]).unwrap(), "connections") == 1,
    );
    // Freeze the server: its sockets stay open but nothing is answered.
    server.signal(rustix::process::Signal::STOP);
    eventually(
        "the silent connection was never closed",
        Duration::from_secs(20),
        || number(&client.ctl(&["status"]).unwrap(), "connections") == 0,
    );
    server.signal(rustix::process::Signal::CONT);
    eventually(
        "the client never reconnected",
        Duration::from_secs(30),
        || number(&client.ctl(&["status"]).unwrap(), "connections") == 1,
    );
}
