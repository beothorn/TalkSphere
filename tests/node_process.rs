//! Two separately operated node processes exercise paid Storage through real authenticated TCP,
//! the owner-only control socket and durable restart: AT-E01 and the failure cases around it.
#![cfg(target_os = "linux")]
mod support;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use support::*;

// AT-E01: a paid store of "abc", a provider restart, then retrieve, renew, check and remove under
// current terms, with one exact charge per paid operation and no optional offering advertised.
#[test]
fn at_e01_minimum_useful_node_between_two_processes() {
    let dir = Dir::new("e01");
    let mut a = Node::init(&dir.0, "a", None);
    let mut b = Node::init(&dir.0, "b", None);
    let port = free_port();
    a.start(None);
    b.start(Some(port));
    fund(&b, &a, 100_000);
    let contact = b.contact(port);
    // Authenticate, read the Board and definition: exactly one provided offering, the Storage one.
    let board = a.ctl(&["fetch", "--contact", &contact]).unwrap();
    let provided = board["provided"].as_array().unwrap();
    assert_eq!(provided.len(), 1);
    assert_eq!(provided[0]["id"], "storage-standard");
    assert_eq!(provided[0]["service"], "cr2se.storage");
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
            "100",
        ])
        .unwrap();
    assert_eq!(
        text(&stored, "contentHash"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let lease = text(&stored, "leaseId");
    let store_price = number(&stored, "price");
    assert_eq!(store_price, 92);
    assert_eq!(a.ctl(&["leases"]).unwrap().as_array().unwrap().len(), 1);
    // Restart the provider on the same port; its durable owners recover before it serves.
    b.stop();
    b.start(Some(port));
    let output = dir.0.join("retrieved");
    let got = a
        .ctl(&[
            "retrieve",
            "--lease",
            &lease,
            "--max-spend",
            "10",
            "--output",
            output.to_str().unwrap(),
        ])
        .unwrap();
    assert_eq!(std::fs::read(&output).unwrap(), b"abc");
    assert_eq!(got["wholeObjectHashChecked"], true);
    let retrieve_price = number(&got, "price");
    let renewed = a
        .ctl(&[
            "renew",
            "--lease",
            &lease,
            "--seconds",
            "30",
            "--max-spend",
            "100",
        ])
        .unwrap();
    assert_eq!(
        number(&renewed, "expiresAt"),
        number(&stored, "expiresAt") + 30
    );
    let renew_price = number(&renewed, "price");
    let checked = a
        .ctl(&[
            "check",
            "--lease",
            &lease,
            "--max-spend",
            "10",
            "--expected-file",
            object.to_str().unwrap(),
        ])
        .unwrap();
    assert_eq!(checked["outcome"], "pass");
    let removed = a
        .ctl(&["remove", "--lease", &lease, "--max-spend", "10"])
        .unwrap();
    assert_eq!(removed["removed"], true);
    let remove_price = number(&removed, "price");
    // One exact charge per paid operation: both owners agree on the total.
    let spent = store_price + retrieve_price + renew_price + 3 + remove_price;
    assert_eq!(number(&balance(&a, &b), "owned"), 100_000 - spent);
    assert_eq!(number(&balance(&b, &a), "issued"), 100_000 - spent);
    // The removed lease is unavailable and charges nothing further.
    assert!(
        a.ctl(&["retrieve", "--lease", &lease, "--max-spend", "10"])
            .is_err()
    );
    assert_eq!(number(&balance(&a, &b), "owned"), 100_000 - spent);
    assert_eq!(number(&balance(&b, &a), "issued"), 100_000 - spent);
    a.stop();
    b.stop();
}

// A TCP relay between a requester and a provider that counts provider-to-requester bytes and can
// cut the connection once a byte threshold is reached, to lose a reply at a chosen point.
struct Proxy {
    port: u16,
    to_requester: Arc<AtomicUsize>,
    cut_at: Arc<AtomicUsize>,
    cut: mpsc::Receiver<()>,
    stop: Arc<AtomicBool>,
}
impl Proxy {
    // Relay every accepted connection to `target`; the threshold starts out unreachable.
    fn start(target: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let (to_requester, cut_at) = (
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(usize::MAX)),
        );
        let stop = Arc::new(AtomicBool::new(false));
        let (cut_tx, cut) = mpsc::channel();
        let (count, limit, halt) = (to_requester.clone(), cut_at.clone(), stop.clone());
        std::thread::spawn(move || {
            while !halt.load(Ordering::SeqCst) {
                let Ok((client, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                };
                client.set_nonblocking(false).unwrap();
                let server = TcpStream::connect(("127.0.0.1", target)).unwrap();
                let (mut c2s_in, mut c2s_out) =
                    (client.try_clone().unwrap(), server.try_clone().unwrap());
                std::thread::spawn(move || {
                    let mut buffer = [0u8; 4096];
                    while let Ok(n) = c2s_in.read(&mut buffer) {
                        if n == 0 || c2s_out.write_all(&buffer[..n]).is_err() {
                            break;
                        }
                    }
                    let _ = c2s_out.shutdown(std::net::Shutdown::Both);
                });
                let (mut s2c_in, mut s2c_out) = (server, client);
                let (count, limit, cut_tx) = (count.clone(), limit.clone(), cut_tx.clone());
                std::thread::spawn(move || {
                    let mut buffer = [0u8; 4096];
                    while let Ok(n) = s2c_in.read(&mut buffer) {
                        if n == 0 {
                            break;
                        }
                        // Forward only the bytes below the threshold, then cut both directions.
                        let before = count.load(Ordering::SeqCst);
                        let allowed = limit.load(Ordering::SeqCst).saturating_sub(before).min(n);
                        if allowed > 0 && s2c_out.write_all(&buffer[..allowed]).is_err() {
                            break;
                        }
                        count.fetch_add(allowed, Ordering::SeqCst);
                        if allowed < n {
                            let _ = cut_tx.send(());
                            break;
                        }
                    }
                    let _ = s2c_out.shutdown(std::net::Shutdown::Both);
                    let _ = s2c_in.shutdown(std::net::Shutdown::Both);
                });
            }
        });
        Self {
            port,
            to_requester,
            cut_at,
            cut,
            stop,
        }
    }
    fn seen(&self) -> usize {
        self.to_requester.load(Ordering::SeqCst)
    }
}
impl Drop for Proxy {
    // Stop accepting; relay threads end with their sockets.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

// Run fetch and one store through a counting relay, returning the provider-to-requester bytes
// seen before the store and after it. Both nodes are fresh, so the byte counts are comparable.
fn calibrate(dir: &Dir) -> (usize, usize) {
    let mut a = Node::init(&dir.0, "ca", None);
    let mut b = Node::init(&dir.0, "cb", None);
    a.start(None);
    b.start(None);
    let proxy = Proxy::start(b.port);
    fund(&b, &a, 100_000);
    let contact = b.contact(proxy.port);
    a.ctl(&["fetch", "--contact", &contact]).unwrap();
    let before = proxy.seen();
    let object = dir.0.join("calibration");
    std::fs::write(&object, b"abc").unwrap();
    a.ctl(&[
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
    .unwrap();
    let after = proxy.seen();
    a.stop();
    b.stop();
    (before, after)
}

// AT-E04 at process level, part one: the connection is cut after the provider committed the store
// but before the requester received the complete result. The requester reports an unknown outcome,
// records it, creates no replacement store, and the provider charged exactly once.
#[test]
fn lost_store_result_is_unknown_and_never_replaced() {
    let dir = Dir::new("lost");
    let (before, after) = calibrate(&dir);
    assert!(after > before + 64, "calibration saw no store traffic");
    let mut a = Node::init(&dir.0, "a", None);
    let mut b = Node::init(&dir.0, "b", None);
    a.start(None);
    b.start(None);
    let proxy = Proxy::start(b.port);
    fund(&b, &a, 100_000);
    let contact = b.contact(proxy.port);
    a.ctl(&["fetch", "--contact", &contact]).unwrap();
    // Cut just before the end of the result so the provider has committed but the requester has not.
    proxy
        .cut_at
        .store(proxy.seen() + (after - before) - 24, Ordering::SeqCst);
    let object = dir.0.join("object");
    std::fs::write(&object, b"abc").unwrap();
    let error = a
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
    assert!(error.contains("unknown_outcome"), "{error}");
    proxy.cut.recv_timeout(Duration::from_secs(5)).unwrap();
    // The requester recorded the unknown outcome and holds no lease; nothing was retried.
    let status = a.ctl(&["status"]).unwrap();
    assert_eq!(number(&status, "unknownOutcomes"), 1);
    assert_eq!(number(&status, "leases"), 0);
    let unknown = a.ctl(&["outcomes", "--unknown"]).unwrap();
    assert_eq!(unknown.as_array().unwrap().len(), 1);
    assert_eq!(unknown[0]["op"], "store");
    // The provider committed and charged exactly once.
    assert_eq!(number(&balance(&b, &a), "issued"), 100_000 - 92);
    // The unknown outcome survives a requester restart and is still reported, not retried.
    a.stop();
    a.start(None);
    assert_eq!(
        a.ctl(&["outcomes", "--unknown"])
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(number(&a.ctl(&["status"]).unwrap(), "leases"), 0);
    assert_eq!(number(&balance(&b, &a), "issued"), 100_000 - 92);
    a.stop();
    b.stop();
}

// AT-E04 at process level, part two: the provider crashes right after committing. After restart its
// recovered state shows exactly one charge and no replayed work, and it serves again.
#[test]
fn provider_crash_after_commit_applies_once() {
    let dir = Dir::new("crash");
    let (before, after) = calibrate(&dir);
    let mut a = Node::init(&dir.0, "a", None);
    let mut b = Node::init(&dir.0, "b", None);
    let port = free_port();
    a.start(None);
    b.start(Some(port));
    let proxy = Proxy::start(port);
    fund(&b, &a, 100_000);
    let contact = b.contact(proxy.port);
    a.ctl(&["fetch", "--contact", &contact]).unwrap();
    proxy
        .cut_at
        .store(proxy.seen() + (after - before) - 24, Ordering::SeqCst);
    let object = dir.0.join("object");
    std::fs::write(&object, b"abc").unwrap();
    let error = a
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
    assert!(error.contains("unknown_outcome"), "{error}");
    b.crash();
    b.start(Some(port));
    // Recovery replays nothing: exactly one charge, and the provider is serving again.
    assert_eq!(number(&balance(&b, &a), "issued"), 100_000 - 92);
    assert_eq!(number(&a.ctl(&["status"]).unwrap(), "leases"), 0);
    let direct = b.contact(port);
    assert_eq!(
        a.ctl(&["fetch", "--contact", &direct]).unwrap()["provided"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // A new store is a separate, explicitly authorized lease with its own charge.
    let stored = a
        .ctl(&[
            "store",
            "--contact",
            &direct,
            "--file",
            object.to_str().unwrap(),
            "--retention-seconds",
            "60",
            "--max-spend",
            "100",
        ])
        .unwrap();
    assert_eq!(number(&stored, "price"), 92);
    assert_eq!(number(&balance(&b, &a), "issued"), 100_000 - 2 * 92);
    a.stop();
    b.stop();
}

// Foreign identities, stale guards and expiry between nodes. A third identity cannot see or use
// another requester's lease, and stale and expired requests are refused without charge.
#[test]
fn unauthorized_stale_and_expired_requests() {
    let dir = Dir::new("guards");
    let (mut a, mut b, mut c) = (
        Node::init(&dir.0, "a", None),
        Node::init(&dir.0, "b", None),
        Node::init(&dir.0, "c", None),
    );
    let port = free_port();
    a.start(None);
    b.start(Some(port));
    c.start(None);
    fund(&b, &a, 200);
    fund(&b, &c, 1_000);
    let contact = b.contact(port);
    let object = dir.0.join("object");
    std::fs::write(&object, b"abc").unwrap();
    let file = object.to_str().unwrap();
    let store = |node: &Node, retention: &str| {
        node.ctl(&[
            "store",
            "--contact",
            &contact,
            "--file",
            file,
            "--retention-seconds",
            retention,
            "--max-spend",
            "100",
        ])
    };
    let first = store(&a, "60").unwrap();
    let lease = text(&first, "leaseId");
    // A foreign requester cannot use or see the lease, and its failed attempt costs nothing.
    let before = number(&balance(&b, &c), "issued");
    let error = c
        .ctl(&["retrieve", "--lease", &lease, "--max-spend", "10"])
        .unwrap_err();
    assert!(error.contains("denied"), "{error}");
    assert_eq!(number(&balance(&b, &c), "issued"), before);
    // The same content bought by the foreign requester is a separate lease with its own identifier.
    let other = store(&c, "60").unwrap();
    assert_ne!(text(&other, "leaseId"), lease);
    // A renewal with a stale expected expiry is refused before any charge.
    let owned = number(&balance(&a, &b), "owned");
    let stale = number(&first, "expiresAt") - 1;
    let error = a
        .ctl(&[
            "renew",
            "--lease",
            &lease,
            "--seconds",
            "10",
            "--max-spend",
            "100",
            "--expected-expiry",
            &stale.to_string(),
        ])
        .unwrap_err();
    assert!(
        error.contains("refused") || error.contains("denied"),
        "{error}"
    );
    assert_eq!(number(&balance(&a, &b), "owned"), owned);
    // A short lease that has expired refuses every operation without charge.
    let short = store(&a, "1").unwrap();
    let short_lease = text(&short, "leaseId");
    let owned = number(&balance(&a, &b), "owned");
    std::thread::sleep(Duration::from_secs(3));
    for args in [
        vec!["retrieve", "--lease", &short_lease, "--max-spend", "10"],
        vec![
            "check",
            "--lease",
            &short_lease,
            "--max-spend",
            "10",
            "--expected-file",
            file,
        ],
    ] {
        let error = a.ctl(&args).unwrap_err();
        assert!(
            error.contains("unavailable") || error.contains("refused"),
            "{error}"
        );
    }
    assert_eq!(number(&balance(&a, &b), "owned"), owned);
    a.stop();
    b.stop();
    c.stop();
}

// Requests for absent optional families never reach acceptance and charge nothing; the Board lists
// only the working Storage offering, and a requester-only node advertises none.
#[test]
fn absent_optional_families_are_refused_without_charge() {
    let dir = Dir::new("absent");
    let mut a = Node::init(&dir.0, "a", Some(r#"{"version":1,"offering":null}"#));
    let mut b = Node::init(&dir.0, "b", None);
    let port = free_port();
    a.start(None);
    b.start(Some(port));
    fund(&b, &a, 1_000);
    let board = a.ctl(&["fetch", "--contact", &b.contact(port)]).unwrap();
    let ids: Vec<_> = board["provided"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["service"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["cr2se.storage"]);
    let own_board = b.ctl(&["fetch", "--contact", &a.contact(a.port)]).unwrap();
    assert!(
        own_board["provided"].as_array().unwrap().is_empty(),
        "a requester-only node advertises nothing"
    );
    let before = number(&balance(&a, &b), "owned");
    for family in ["compute", "message", "discover", "files", "feeds", "pages"] {
        let out = Command::new(BIN)
            .args(["node-ctl", "--node-root", a.root.to_str().unwrap(), family])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "{family} is not a node-ctl action"
        );
    }
    assert_eq!(number(&balance(&a, &b), "owned"), before);
    a.stop();
    b.stop();
}

// Concurrent stores share finite credits: with credits for one 100-credit reservation only one of
// two simultaneous stores succeeds, the other is refused, and both owners agree on the exact total.
#[test]
fn concurrent_stores_never_overspend_finite_credits() {
    let dir = Dir::new("race");
    let (mut a, mut b) = (Node::init(&dir.0, "a", None), Node::init(&dir.0, "b", None));
    let port = free_port();
    a.start(None);
    b.start(Some(port));
    fund(&b, &a, 150);
    let contact = b.contact(port);
    let object = dir.0.join("object");
    std::fs::write(&object, b"abc").unwrap();
    a.ctl(&["fetch", "--contact", &contact]).unwrap();
    let store = || {
        a.ctl(&[
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
    };
    let (r1, r2) = std::thread::scope(|scope| {
        let t1 = scope.spawn(store);
        let t2 = scope.spawn(store);
        (t1.join().unwrap(), t2.join().unwrap())
    });
    let results = [r1, r2];
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "{results:?}"
    );
    let refused = results.iter().find_map(|r| r.as_ref().err()).unwrap();
    assert!(refused.contains("insufficient"), "{refused}");
    assert_eq!(number(&balance(&a, &b), "owned"), 150 - 92);
    assert_eq!(number(&balance(&b, &a), "issued"), 150 - 92);
    assert_eq!(a.ctl(&["leases"]).unwrap().as_array().unwrap().len(), 1);
    a.stop();
    b.stop();
}

// Send SIGTERM and wait for exit, returning whether the node stopped cleanly and how long it took.
fn terminate(node: &mut Node) -> (bool, Duration) {
    terminate_child(node.child.take().unwrap())
}

// Same, for a child process that was already taken from its node.
fn terminate_child(mut child: Child) -> (bool, Duration) {
    let started = std::time::Instant::now();
    rustix::process::kill_process(
        rustix::process::Pid::from_raw(child.id() as i32).unwrap(),
        rustix::process::Signal::TERM,
    )
    .unwrap();
    let status = child.wait().unwrap();
    (status.success(), started.elapsed())
}

// Shutdown while a large upload is in flight: the provider stops within its drain time, and after
// restart its recovered state is consistent: the store either completed with one charge or never
// happened, and a lease the requester cataloged is served.
#[test]
fn shutdown_during_an_upload_leaves_consistent_state() {
    let dir = Dir::new("drain-upload");
    let (mut a, mut b) = (Node::init(&dir.0, "a", None), Node::init(&dir.0, "b", None));
    let port = free_port();
    a.start(None);
    b.start(Some(port));
    fund(&b, &a, 1_000_000);
    let contact = b.contact(port);
    a.ctl(&["fetch", "--contact", &contact]).unwrap();
    let object = dir.0.join("big");
    std::fs::write(&object, vec![7u8; 16_000]).unwrap();
    let price = 16_000u64 * 61 / 2;
    let stored = std::thread::scope(|scope| {
        let upload = scope.spawn(|| {
            a.ctl(&[
                "store",
                "--contact",
                &contact,
                "--file",
                object.to_str().unwrap(),
                "--retention-seconds",
                "60",
                "--max-spend",
                &(price + 10).to_string(),
            ])
        });
        std::thread::sleep(Duration::from_millis(40));
        let (clean, took) = terminate(&mut b);
        assert!(clean, "the provider did not stop cleanly while draining");
        assert!(took < Duration::from_secs(8), "shutdown took {took:?}");
        upload.join().unwrap()
    });
    b.start(Some(port));
    let charged = 1_000_000 - number(&balance(&b, &a), "issued");
    match stored {
        Ok(lease) => {
            assert_eq!(charged, price);
            let id = text(&lease, "leaseId");
            let got = a
                .ctl(&[
                    "retrieve",
                    "--lease",
                    &id,
                    "--length",
                    "4",
                    "--max-spend",
                    "100",
                ])
                .unwrap();
            assert_eq!(text(&got, "data"), "07070707");
        }
        Err(error) => {
            // Either nothing was committed, or the outcome is unknown and never retried.
            assert!(charged == 0 || charged == price, "{charged}");
            assert!(
                error.contains("unavailable")
                    || error.contains("unknown_outcome")
                    || error.contains("refused"),
                "{error}"
            );
            assert_eq!(number(&a.ctl(&["status"]).unwrap(), "leases"), 0);
        }
    }
    assert!(terminate(&mut a).0);
    b.stop();
}

// A control request waiting on an unresponsive peer is released when the node stops, and the node
// exits promptly and cleanly instead of waiting out the request deadline.
#[test]
fn shutdown_releases_a_pending_control_request() {
    let dir = Dir::new("drain-pending");
    let mut a = Node::init(&dir.0, "a", None);
    let dead = Node::init(&dir.0, "dead", None);
    a.start(None);
    // A listener that accepts connections but never speaks, so authentication cannot complete.
    let silent = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = silent.local_addr().unwrap().port();
    let contact = dead.contact(port);
    let child = a.child.take().unwrap();
    let (request_result, stop) = std::thread::scope(|scope| {
        let request = scope.spawn(|| a.ctl(&["fetch", "--contact", &contact]));
        std::thread::sleep(Duration::from_millis(300));
        let stop = terminate_child(child);
        (request.join().unwrap(), stop)
    });
    assert!(stop.0, "the node did not stop cleanly");
    assert!(
        stop.1 < Duration::from_secs(8),
        "shutdown took {:?}",
        stop.1
    );
    let error = request_result.unwrap_err();
    assert!(
        error.contains("stopping") || error.contains("unavailable") || error.contains("busy"),
        "{error}"
    );
    drop(silent);
    // The node restarts cleanly with its owners intact.
    a.start(None);
    assert_eq!(number(&a.ctl(&["status"]).unwrap(), "leases"), 0);
    a.stop();
}

// Logs and control replies from a full flow, at trace level, never contain object content, key
// material or lease metadata a node has no right to know; stdout carries only intentional lines.
#[test]
fn logs_and_replies_exclude_secrets_and_foreign_metadata() {
    let dir = Dir::new("privacy");
    let (mut a, mut b, mut c) = (
        Node::init(&dir.0, "a", None),
        Node::init(&dir.0, "b", None),
        Node::init(&dir.0, "c", None),
    );
    let port = free_port();
    a.start_with(None, Some("trace"));
    b.start_with(Some(port), Some("trace"));
    c.start_with(None, Some("trace"));
    fund(&b, &a, 10_000);
    fund(&b, &c, 10_000);
    let contact = b.contact(port);
    let secret = b"TOP-SECRET-OBJECT-CONTENT-ABCDEFGHIJ";
    let object = dir.0.join("object");
    std::fs::write(&object, secret).unwrap();
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
            "2000",
        ])
        .unwrap();
    let lease = text(&stored, "leaseId");
    // The requester's own check and retrieval, and a foreign identity's failed attempts.
    a.ctl(&[
        "check",
        "--lease",
        &lease,
        "--max-spend",
        "10",
        "--expected-file",
        object.to_str().unwrap(),
        "--offset",
        "3",
    ])
    .unwrap();
    let output = dir.0.join("out");
    a.ctl(&[
        "retrieve",
        "--lease",
        &lease,
        "--max-spend",
        "100",
        "--output",
        output.to_str().unwrap(),
    ])
    .unwrap();
    assert!(
        c.ctl(&["retrieve", "--lease", &lease, "--max-spend", "10"])
            .is_err()
    );
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let forbidden = [
        String::from_utf8(secret.to_vec()).unwrap(),
        hex(secret),
        hex(&secret[3..4]) + "\"", // the privately held expected byte as a quoted value
    ];
    let key_files: Vec<_> = std::fs::read_dir(a.root.join("identity"))
        .unwrap()
        .map(|e| std::fs::read(e.unwrap().path()).unwrap())
        .filter(|bytes| bytes.len() >= 32)
        .collect();
    let (a_path, b_path, c_path) = (a.log_path(), b.log_path(), c.log_path());
    let a_out = a.lines.lock().unwrap().join("\n");
    a.stop();
    b.stop();
    c.stop();
    for (name, path) in [("a", a_path), ("b", b_path), ("c", c_path)] {
        let log = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&log);
        assert!(log.len() > 10_000, "{name} produced no trace output");
        for needle in &forbidden[..2] {
            assert!(
                !text.contains(needle.as_str()),
                "{name} logged object content"
            );
        }
        for key in &key_files {
            let window = &key[..32];
            assert!(
                !log.windows(32).any(|w| w == window),
                "{name} logged private key bytes"
            );
        }
        if name == "c" {
            assert!(
                !text.contains(&lease),
                "the foreign node logged another requester's lease id"
            );
        }
    }
    // Standard output holds only the intentional lifecycle lines.
    for line in a_out.lines() {
        assert!(
            line.starts_with("ready ")
                || line.starts_with("authenticated ")
                || line == "connection-closed"
                || line == "stopped",
            "unexpected stdout line {line:?}"
        );
    }
}

// The live connection registry follows authenticated connections in both directions: a requester that
// connects outward appears at the provider while connected, the provider sees it as a peer, and both
// sides forget it after the connection closes.
#[test]
fn live_connections_are_tracked_and_forgotten() {
    let dir = Dir::new("live");
    let mut a = Node::init(&dir.0, "a", None);
    let mut b = Node::init(&dir.0, "b", None);
    b.start(None);
    a.start(None);
    assert_eq!(number(&b.ctl(&["status"]).unwrap(), "connections"), 0);
    let contact = b.contact(b.port);
    a.ctl(&["fetch", "--contact", &contact]).unwrap();
    let at_b = b.ctl(&["status"]).unwrap();
    assert_eq!(number(&at_b, "connections"), 1);
    assert_eq!(at_b["connectedPeers"][0].as_str().unwrap(), a.id);
    let at_a = a.ctl(&["status"]).unwrap();
    assert_eq!(at_a["connectedPeers"][0].as_str().unwrap(), b.id);
    a.stop();
    // The provider notices the close without any request from the closed peer.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while number(&b.ctl(&["status"]).unwrap(), "connections") != 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "the closed connection was never forgotten"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        b.ctl(&["status"]).unwrap()["connectedPeers"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
