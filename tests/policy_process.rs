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

// A provider with a bootstrap policy gives each new identity one finite grant, within a total budget:
// two peers get credits, the third finds the budget used up, a restarted provider never repeats a
// grant, and the granted peer can spend them once it records the credits it holds.
#[test]
fn bootstrap_policy_grants_once_per_identity_within_the_budget() {
    let dir = Dir::new("bootstrap");
    let mut provider = Node::init(&dir.0, "provider", None);
    let mut peers: Vec<Node> = ["a", "b", "c"]
        .iter()
        .map(|n| Node::init(&dir.0, n, None))
        .collect();
    provider.set_policy(
        r#"{"version":1,"credit":{"bootstrapCredits":150,"bootstrapTrust":1,"bootstrapBudget":300}}"#,
    );
    let port = free_port();
    provider.start(Some(port));
    let contact = provider.contact(port);
    for peer in &mut peers {
        peer.start(None);
        peer.ctl(&["fetch", "--contact", &contact]).unwrap();
    }
    let issued = |provider: &Node, peer: &Node| number(&balance(provider, peer), "issued");
    let trusted = |provider: &Node, peer: &Node| number(&balance(provider, peer), "trust");
    assert_eq!(
        (issued(&provider, &peers[0]), trusted(&provider, &peers[0])),
        (150, 1)
    );
    assert_eq!(
        (issued(&provider, &peers[1]), trusted(&provider, &peers[1])),
        (150, 1)
    );
    assert_eq!(
        (issued(&provider, &peers[2]), trusted(&provider, &peers[2])),
        (0, 0)
    );
    // The first peer records what it holds and spends part of it.
    let a = &peers[0];
    a.ctl(&["own", "--peer", &provider.id, "--amount", "150"])
        .unwrap();
    a.ctl(&["trust", "--peer", &provider.id, "--millionths", "1"])
        .unwrap();
    let object = dir.0.join("object");
    std::fs::write(&object, b"abc").unwrap();
    let stored = a
        .ctl(&[
            "store",
            "--contact",
            &contact,
            "--file",
            object.to_str().unwrap(),
            "--retention-seconds",
            "60",
            "--max-spend",
            "150",
        ])
        .unwrap();
    let remaining = 150 - number(&stored, "price");
    assert_eq!(issued(&provider, a), remaining);
    // A restarted provider remembers its grants: reconnecting changes nothing, and the third peer
    // still gets nothing because the budget stays used up.
    provider.crash();
    provider.start(Some(port));
    for peer in &peers {
        peer.ctl(&["fetch", "--contact", &contact]).unwrap();
    }
    assert_eq!(issued(&provider, &peers[0]), remaining);
    assert_eq!(issued(&provider, &peers[1]), 150);
    assert_eq!(issued(&provider, &peers[2]), 0);
}

// A server that wants Storage pays a client in its own credits for holding one queued object: the client
// fulfils the wanted offering over its own connection and earns exactly the published credits, the server
// keeps the lease as one of its replicas, a second offer with nothing queued does nothing, and a finite
// issuance budget stops further contributions without charge. Everything survives a server restart.
#[test]
fn a_client_earns_credits_by_fulfilling_wanted_storage() {
    let dir = Dir::new("wanted");
    let mut server = Node::init(
        &dir.0,
        "server",
        Some(r#"{"version":1,"credits":{"issuance":8}}"#),
    );
    let mut client = Node::init(&dir.0, "client", None);
    server.set_policy(
        r#"{"version":1,"credit":{"wanted":{"creditsPerObject":5,"maximumObjectBytes":1024,"retentionSeconds":300}}}"#,
    );
    let port = free_port();
    server.start(Some(port));
    client.start(None);
    let contact = server.contact(port);
    let (first, second) = (dir.0.join("first"), dir.0.join("second"));
    std::fs::write(&first, b"replica one").unwrap();
    std::fs::write(&second, b"replica two").unwrap();
    // Without a wanted offering a node refuses to queue work.
    assert!(
        client
            .ctl(&["wanted-add", "--file", first.to_str().unwrap()])
            .is_err()
    );
    assert_eq!(server.ctl(&["status"]).unwrap()["wanted"]["queued"], 0);
    server
        .ctl(&["wanted-add", "--file", first.to_str().unwrap()])
        .unwrap();
    server
        .ctl(&["wanted-add", "--file", second.to_str().unwrap()])
        .unwrap();
    // The provider of the contribution must trust the publisher whose credits it will hold.
    client
        .ctl(&["trust", "--peer", &server.id, "--millionths", "1"])
        .unwrap();
    let fulfilled = client.ctl(&["fulfil", "--contact", &contact]).unwrap();
    assert_eq!(fulfilled["fulfilled"], true);
    assert_eq!(number(&balance(&client, &server), "owned"), 5);
    // The server settles and catalogs the replica after the client's exchange ends.
    eventually(
        "the server never settled and cataloged the replica",
        Duration::from_secs(10),
        || {
            let status = server.ctl(&["status"]).unwrap();
            status["wanted"]["inFlight"] == 0
                && server
                    .ctl(&["leases"])
                    .unwrap()
                    .as_array()
                    .is_some_and(|l| l.len() == 1)
        },
    );
    assert_eq!(number(&balance(&server, &client), "issued"), 5);
    let leases = server.ctl(&["leases"]).unwrap();
    assert_eq!(leases[0]["provider"].as_str().unwrap(), client.id);
    assert_eq!(number(&leases[0], "storePrice"), 5);
    assert_eq!(server.ctl(&["status"]).unwrap()["wanted"]["queued"], 1);
    // The finite issuance budget (8) cannot cover a second contribution of 5: nothing is charged.
    let refused = client.ctl(&["fulfil", "--contact", &contact]).unwrap_err();
    assert!(!refused.is_empty());
    assert_eq!(number(&balance(&client, &server), "owned"), 5);
    assert_eq!(number(&balance(&server, &client), "issued"), 5);
    // State survives a server restart.
    server.crash();
    server.start(Some(port));
    assert_eq!(number(&balance(&server, &client), "issued"), 5);
    assert_eq!(
        server.ctl(&["leases"]).unwrap().as_array().unwrap().len(),
        1
    );
}

// Trust moves only on verified observations and only within the configured steps: a verified
// contribution raises it, a passed check raises it further up to the ceiling, a failed check lowers it,
// and an operator's explicit trust is the baseline the steps start from.
#[test]
fn trust_follows_verified_contributions_and_checks() {
    let dir = Dir::new("trust");
    let mut server = Node::init(&dir.0, "server", None);
    let mut client = Node::init(&dir.0, "client", None);
    server.set_policy(
        r#"{"version":1,
            "credit":{"wanted":{"creditsPerObject":5,"maximumObjectBytes":1024,"retentionSeconds":300}},
            "trust":{"contribution":300000,"checkPassed":200000,"checkFailed":250000,"ceiling":600000}}"#,
    );
    let port = free_port();
    server.start(Some(port));
    client.start(None);
    let contact = server.contact(port);
    let object = dir.0.join("object");
    std::fs::write(&object, b"replica bytes").unwrap();
    server
        .ctl(&["wanted-add", "--file", object.to_str().unwrap()])
        .unwrap();
    client
        .ctl(&["trust", "--peer", &server.id, "--millionths", "1"])
        .unwrap();
    assert_eq!(number(&balance(&server, &client), "trust"), 0);
    client.ctl(&["fulfil", "--contact", &contact]).unwrap();
    let trust = |server: &Node| number(&balance(server, &client), "trust");
    eventually(
        "the verified contribution never raised trust",
        Duration::from_secs(10),
        || trust(&server) == 300_000,
    );
    // The server pays the client's own credits for checks, so it records and trusts them first.
    fund(&client, &server, 100);
    // fund() set the server's trust in the client to 1 millionth, the operator's explicit baseline.
    let leases = server.ctl(&["leases"]).unwrap();
    let lease = leases[0]["leaseId"].as_str().unwrap().to_string();
    let check = |byte: &str| {
        server
            .ctl(&[
                "check",
                "--lease",
                &lease,
                "--max-spend",
                "10",
                "--offset",
                "0",
                "--expected-byte",
                byte,
            ])
            .unwrap()["outcome"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(check("72"), "pass");
    assert_eq!(trust(&server), 200_001);
    assert_eq!(check("72"), "pass");
    assert_eq!(trust(&server), 400_001);
    assert_eq!(check("72"), "pass");
    assert_eq!(trust(&server), 600_000);
    assert_eq!(check("72"), "pass");
    assert_eq!(
        trust(&server),
        600_000,
        "automatic raises stop at the ceiling"
    );
    assert_eq!(check("00"), "fail");
    assert_eq!(trust(&server), 350_000);
}

// Whether any file below `dir` contains `needle`.
fn tree_contains(dir: &std::path::Path, needle: &[u8]) -> bool {
    std::fs::read_dir(dir).unwrap().flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            tree_contains(&path, needle)
        } else {
            std::fs::read(&path).is_ok_and(|bytes| bytes.windows(needle.len()).any(|w| w == needle))
        }
    })
}

// A hosted item is sealed once and placed through the wanted offering at distinct clients until the
// redundancy target is met: a client never receives the same item twice, once the target is reached
// further offers do nothing and cost nothing, and the clients hold only sealed bytes.
#[test]
fn a_hosted_item_is_replicated_to_distinct_clients_up_to_the_target() {
    let dir = Dir::new("replicas");
    let mut server = Node::init(&dir.0, "server", None);
    let mut clients: Vec<Node> = ["c1", "c2", "c3"]
        .iter()
        .map(|n| Node::init(&dir.0, n, None))
        .collect();
    server.set_policy(
        r#"{"version":1,"credit":{"wanted":{"creditsPerObject":5,"maximumObjectBytes":1024,"retentionSeconds":300}},"redundancy":{"target":2}}"#,
    );
    let port = free_port();
    server.start(Some(port));
    let contact = server.contact(port);
    let secret = b"TOP-SECRET-HOSTED-DATA";
    let item = dir.0.join("item");
    std::fs::write(&item, secret).unwrap();
    // Without a wanted offering nothing can be placed.
    let added = server
        .ctl(&["replica-add", "--file", item.to_str().unwrap()])
        .unwrap();
    assert_eq!(number(&added, "sealedBytes"), secret.len() as u64 + 41);
    assert_eq!(number(&added, "target"), 2);
    for client in &mut clients {
        client.start(None);
        client
            .ctl(&["trust", "--peer", &server.id, "--millionths", "1"])
            .unwrap();
    }
    let confirmed =
        |server: &Node| number(&server.ctl(&["replicas"]).unwrap()["items"][0], "confirmed");
    clients[0].ctl(&["fulfil", "--contact", &contact]).unwrap();
    eventually(
        "the first replica was never confirmed",
        Duration::from_secs(10),
        || confirmed(&server) == 1,
    );
    // One copy is not redundancy, whatever the target: the state and the target are reported separately.
    let listed = server.ctl(&["replicas"]).unwrap();
    assert_eq!(listed["items"][0]["state"], "not redundant");
    assert_eq!(listed["items"][0]["targetMet"], false);
    assert_eq!(listed["items"][0]["target"], 2);
    // The same client cannot hold a second copy of the item.
    assert!(clients[0].ctl(&["fulfil", "--contact", &contact]).is_err());
    assert_eq!(confirmed(&server), 1);
    clients[1].ctl(&["fulfil", "--contact", &contact]).unwrap();
    eventually(
        "the second replica was never confirmed",
        Duration::from_secs(10),
        || confirmed(&server) == 2,
    );
    let listed = server.ctl(&["replicas"]).unwrap();
    assert_eq!(listed["items"][0]["state"], "target satisfied");
    assert_eq!(listed["items"][0]["targetMet"], true);
    // The target is met: a third client is refused, nothing is paid and nothing is stored.
    assert!(clients[2].ctl(&["fulfil", "--contact", &contact]).is_err());
    assert_eq!(number(&balance(&clients[2], &server), "owned"), 0);
    assert_eq!(confirmed(&server), 2);
    let listed = server.ctl(&["replicas"]).unwrap();
    let holders: Vec<&str> = listed["items"][0]["replicas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["provider"].as_str().unwrap())
        .collect();
    assert_eq!(holders.len(), 2);
    assert_ne!(holders[0], holders[1]);
    assert!(holders.contains(&clients[0].id.as_str()) && holders.contains(&clients[1].id.as_str()));
    // Replicas hold sealed bytes only; the plaintext is nowhere at the clients or in the server's book.
    assert!(!tree_contains(&clients[0].root, secret));
    assert!(!tree_contains(&clients[1].root, secret));
    assert!(!tree_contains(&server.root.join("replicas"), secret));
    // Everything survives a server restart.
    server.crash();
    server.start(Some(port));
    assert_eq!(confirmed(&server), 2);
}

// The server's policy for the maintenance tests: a short retention, fast checks and early renewal.
const MAINTENANCE_POLICY: &str = r#"{"version":1,
    "credit":{"wanted":{"creditsPerObject":5,"maximumObjectBytes":1024,"retentionSeconds":12}},
    "redundancy":{"target":1,"checkSeconds":3,"staggerPercent":0,"renewBeforeSeconds":6,"maxSpend":1000,"retrySeconds":1},
    "trust":{"checkFailed":300000,"checkPassed":100000,"ceiling":900000}}"#;

// The first replica of the first hosted item, as the server lists it.
fn first_replica(server: &Node) -> Option<serde_json::Value> {
    server.ctl(&["replicas"]).ok()?["items"][0]["replicas"]
        .get(0)
        .cloned()
}

// Place one hosted item at `client` and give the server the holder's credits to pay for maintenance.
fn host_one_item(dir: &Dir, server: &Node, client: &Node, contact: &str) {
    let item = dir.0.join("item");
    std::fs::write(&item, b"maintained replica bytes").unwrap();
    server
        .ctl(&["replica-add", "--file", item.to_str().unwrap()])
        .unwrap();
    client
        .ctl(&["trust", "--peer", &server.id, "--millionths", "1"])
        .unwrap();
    client.ctl(&["fulfil", "--contact", contact]).unwrap();
    eventually(
        "the replica was never confirmed",
        Duration::from_secs(10),
        || first_replica(server).is_some(),
    );
    // The holder's own credits pay for its checks and renewals; the operator funds them explicitly.
    fund(client, server, 5000);
}

// Due checks and renewals are performed over the holder's own connection and paid in its credits: a
// passed check records the time, a renewal extends the retention before it ends, and the replica stays
// counted throughout.
#[test]
fn replicas_are_checked_and_renewed_before_they_expire() {
    let dir = Dir::new("maintain");
    let mut server = Node::init(&dir.0, "server", None);
    let mut client = Node::init(&dir.0, "client", None);
    server.set_policy(MAINTENANCE_POLICY);
    let port = free_port();
    server.start(Some(port));
    client.start(None);
    host_one_item(&dir, &server, &client, &server.contact(port));
    let confirmed = first_replica(&server).unwrap();
    let first_expiry = number(&confirmed, "expiresAt");
    eventually(
        "the replica was never checked",
        Duration::from_secs(20),
        || first_replica(&server).is_some_and(|r| !r["lastChecked"].is_null()),
    );
    eventually(
        "the replica was never renewed",
        Duration::from_secs(30),
        || first_replica(&server).is_some_and(|r| number(&r, "expiresAt") >= first_expiry + 12),
    );
    // Maintenance was paid for in the holder's credits, never for free.
    assert!(number(&balance(&server, &client), "owned") < 5000);
    assert_eq!(
        server.ctl(&["replicas"]).unwrap()["items"][0]["confirmed"],
        1
    );
}

// A replica whose holder never answers is not renewed and reaches the end of its retention: it stops
// counting, the item needs another copy, and the next client that offers receives it.
#[test]
fn an_unreachable_holder_expires_and_the_item_is_replicated_again() {
    let dir = Dir::new("expire");
    let mut server = Node::init(&dir.0, "server", None);
    let (mut gone, mut other) = (
        Node::init(&dir.0, "gone", None),
        Node::init(&dir.0, "other", None),
    );
    server.set_policy(MAINTENANCE_POLICY);
    let port = free_port();
    server.start(Some(port));
    gone.start(None);
    other.start(None);
    let contact = server.contact(port);
    host_one_item(&dir, &server, &gone, &contact);
    other
        .ctl(&["trust", "--peer", &server.id, "--millionths", "1"])
        .unwrap();
    // The only copy is at a holder that disappears: it cannot be renewed or checked.
    gone.crash();
    eventually(
        "the unreachable replica never expired",
        Duration::from_secs(40),
        || first_replica(&server).is_none(),
    );
    assert_eq!(
        server.ctl(&["replicas"]).unwrap()["items"][0]["confirmed"],
        0
    );
    // The item needs a copy again and goes to the next client that offers.
    other.ctl(&["fulfil", "--contact", &contact]).unwrap();
    eventually(
        "the replacement was never confirmed",
        Duration::from_secs(10),
        || {
            first_replica(&server)
                .is_some_and(|r| r["provider"].as_str() == Some(other.id.as_str()))
        },
    );
}

// A replica that fails a conclusive check stops counting and lowers the holder's trust by the policy
// step; the failure is produced by changing the bytes the server expects, so the holder's copy no
// longer matches.
#[test]
fn a_failed_check_removes_the_replica_and_lowers_trust() {
    let dir = Dir::new("failcheck");
    let mut server = Node::init(&dir.0, "server", None);
    let mut client = Node::init(&dir.0, "client", None);
    server.set_policy(MAINTENANCE_POLICY);
    let port = free_port();
    server.start(Some(port));
    client.start(None);
    host_one_item(&dir, &server, &client, &server.contact(port));
    let id = server.ctl(&["replicas"]).unwrap()["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    // Corrupt the server's local sealed bytes, keeping their length, before the next check samples them.
    let sealed = server.root.join("replicas").join(format!("{id}.sealed"));
    let bytes: Vec<u8> = std::fs::read(&sealed).unwrap().iter().map(|b| !b).collect();
    std::fs::write(&sealed, bytes).unwrap();
    eventually(
        "the failed check never removed the replica",
        Duration::from_secs(30),
        || first_replica(&server).is_none(),
    );
    // The failed check is evidence about the holder: its trust fell by the configured step (from the explicit
    // trust the operator set when funding, floored at zero).
    assert_eq!(number(&balance(&server, &client), "trust"), 0);
}
