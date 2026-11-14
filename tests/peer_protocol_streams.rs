//! Real TCP/Identity/Cryptography and Board metadata through the opt-in Peer Protocol adapter.
//! Baseline: https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md
//! These are same-implementation nodes in one process, not full service-node or interoperability evidence.
#![cfg(target_os = "linux")]
use std::{
    collections::BTreeMap,
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};
use talksphere::{
    composition::server::{
        self,
        config::{Config as ServerConfig, Options},
        pump::Signal,
        transport::Transport,
    },
    modules::{
        board,
        identity::api::IdentityId,
        peer_protocol::{
            self as peer, Error,
            contract::Association,
            engine::bridge::{Bridge, Config, Factory, Handler, Output, Scope},
            exchange::{Direction, Exchange, Outcome},
            message::{Kind, Message},
            record::{CONTROL_MAX, Record},
        },
        transport::api::*,
    },
};
#[derive(Default)]
struct Results {
    completed: Vec<(Association, serde_json::Value)>,
    closed: Vec<(u32, StreamOutcome)>,
}
struct Metadata {
    owner: Arc<board::state::Provider>,
    read: Arc<board::authority::Grant>,
    results: Arc<Mutex<Results>>,
    malformed_stream: Option<u32>,
}
impl Factory for Metadata {
    // Explicit fixture policy requests metadata only; no executable family or financial permission is installed.
    fn open(
        &self,
        scope: Scope,
        opener: Direction,
        output: &mut Output,
    ) -> Result<Box<dyn Handler>, Error> {
        let association = scope.association(if opener == Direction::Local {
            scope.local
        } else {
            scope.peer
        })?;
        let mut exchange = Exchange::new(opener);
        if opener == Direction::Local {
            exchange.control(Direction::Local, Kind::BoardGet)?;
            let bytes = if self.malformed_stream == Some(scope.stream) {
                b"{malformed".as_slice()
            } else {
                br#"{"type":"board.get"}"#
            };
            output.enqueue(&Record::Control(bytes.to_vec()))?;
            exchange.end(Direction::Local)?;
            output.end();
        }
        Ok(Box::new(MetadataStream {
            association,
            local: opener == Direction::Local,
            exchange,
            owner: self.owner.clone(),
            read: self.read.clone(),
            results: self.results.clone(),
            provisional: None,
            deadline: scope.deadline,
        }))
    }
}
struct MetadataStream {
    association: Association,
    local: bool,
    exchange: Exchange,
    owner: Arc<board::state::Provider>,
    read: Arc<board::authority::Grant>,
    results: Arc<Mutex<Results>>,
    provisional: Option<serde_json::Value>,
    deadline: Instant,
}
impl Handler for MetadataStream {
    // Validate actual controls/order and serve the actual Board public contract under separate read authority.
    fn record(&mut self, record: Record, output: &mut Output) -> Result<(), Error> {
        let Record::Control(bytes) = record else {
            return Err(Error::Invalid);
        };
        let message = Message::parse(&bytes, 65536)?;
        self.exchange.control(Direction::Remote, message.kind())?;
        if self.local {
            if message.kind() != Kind::Board {
                return Err(Error::Invalid);
            }
            self.provisional = Some(message.value()["board"].clone());
        } else {
            let context = board::contract::Context {
                grant: &self.read,
                deadline: self.deadline,
                cancelled: false,
            };
            let response = peer::metadata::respond(&self.owner, &context, &message)?;
            let Record::Control(raw) = &response else {
                return Err(Error::Invalid);
            };
            self.exchange
                .control(Direction::Local, Message::parse(raw, 65536)?.kind())?;
            output.enqueue(&response)?;
        }
        Ok(())
    }
    // Complete both application directions without exposing the requester's provisional metadata.
    fn remote_end(&mut self, output: &mut Output) -> Result<(), Error> {
        self.exchange.end(Direction::Remote)?;
        if !self.local {
            self.exchange.end(Direction::Local)?;
            output.end();
        }
        Ok(())
    }
    // Metadata responses fit in one bounded queue and require no incremental body production.
    fn writable(&mut self, _: &mut Output) -> Result<(), Error> {
        Ok(())
    }
    // Publish a requester observation only after actual normal Transport completion and valid application ordering.
    fn closed(&mut self, outcome: StreamOutcome) -> Result<(), Error> {
        let normal = matches!(outcome, StreamOutcome::Completed);
        let status = self.exchange.close(normal)?;
        let mut results = self.results.lock().unwrap();
        assert!(results.closed.len() < 32);
        results
            .closed
            .push((self.association.operation.stream, outcome));
        if self.local && status == Outcome::Complete {
            results.completed.push((
                self.association,
                self.provisional.take().ok_or(Error::Invalid)?,
            ));
        }
        Ok(())
    }
}
struct WakeNode(Signal);
impl Wake for WakeNode {
    // Use the same notification source for real socket readiness and worker completions.
    fn wake(self: Arc<Self>) {
        self.0.notify();
    }
}
struct Node {
    transport: Transport,
    bridge: Bridge,
    signal: Signal,
    waker: Waker,
    results: Arc<Mutex<Results>>,
    address: std::net::SocketAddr,
    authenticated: usize,
}
impl Node {
    // Load real protected credentials and publish an explicitly empty Board using the actual node identity.
    fn new(path: &Path, signal: Signal, malformed_stream: Option<u32>) -> Self {
        assert!(
            Command::new(env!("CARGO_BIN_EXE_talksphere"))
                .args(["identity-init", "--identity-store"])
                .arg(path)
                .output()
                .unwrap()
                .status
                .success()
        );
        let config = ServerConfig::new(Options::local(path.into()).unwrap()).unwrap();
        let credentials = server::credentials::load(&config, false).unwrap();
        let mut transport = server::transport::compose(config, credentials).unwrap();
        let local = IdentityId(transport.local_id.0);
        let authority = Arc::new(board::authority::Authority::new());
        let owner = Arc::new(
            board::state::Provider::new(
                local,
                authority.clone(),
                Default::default(),
                Arc::new(board::semantics::Unavailable),
            )
            .unwrap(),
        );
        let publish = authority
            .grant(local, board::authority::Action::Publish, 2_000_000)
            .unwrap();
        let prepared = board::prepared::Prepared::new(
            local,
            br#"{"version":1,"providedServices":[],"wantedServices":[]}"#,
            &BTreeMap::new(),
            Default::default(),
            &board::semantics::Unavailable,
        )
        .unwrap();
        owner
            .publish(
                &board::contract::Context {
                    grant: &publish,
                    deadline: Instant::now() + Duration::from_secs(5),
                    cancelled: false,
                },
                prepared,
            )
            .unwrap();
        let read = Arc::new(
            authority
                .grant(local, board::authority::Action::Read, 2_000_000)
                .unwrap(),
        );
        let results = Arc::new(Mutex::new(Results::default()));
        let factory = Arc::new(Metadata {
            owner,
            read,
            results: results.clone(),
            malformed_stream,
        });
        let bridge = Bridge::register(
            &mut transport.driver,
            Config {
                connections: 4,
                exchanges: 4,
                workers: 2,
                input_bytes: 131072,
                output_bytes: CONTROL_MAX + 5,
                memory_bytes: 32_000_000,
                controls_per_second: 32,
                lifetime: Duration::from_secs(5),
            },
            factory,
        )
        .unwrap();
        let address = transport.listen().unwrap().address;
        let waker = Waker::from(Arc::new(WakeNode(signal.clone())));
        Self {
            transport,
            bridge,
            signal,
            waker,
            results,
            address,
            authenticated: 0,
        }
    }
    // Poll real Transport before the adapter, including turns without application events or writable callbacks.
    fn poll(&mut self) {
        let mut context = Context::from_waker(&self.waker);
        for _ in 0..64 {
            let Poll::Ready(Some(event)) = self.transport.driver.poll_event(&mut context) else {
                break;
            };
            self.transport.observe(&event).unwrap();
            if matches!(
                self.bridge
                    .event(&mut self.transport.driver, event)
                    .unwrap(),
                Some(Event::Authenticated(_))
            ) {
                self.authenticated += 1;
            }
        }
        self.bridge
            .poll(&mut self.transport.driver, &mut context)
            .unwrap();
    }
}
// Drive actual readiness notifications with an absolute failure deadline; no fixed sleeps or port assumptions.
fn until(a: &mut Node, b: &mut Node, done: impl Fn(&Node, &Node) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        a.poll();
        b.poll();
        if done(a, b) {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "real Peer Protocol metadata exchange stalled"
        );
        a.signal.wait(remaining).unwrap();
    }
}
// Actual TCP/authenticated identities/crypto and Board metadata complete; a malformed sibling receives only PROTOCOL_ERROR.
#[test]
fn authenticated_metadata_and_malformed_stream_isolation_over_real_tcp() {
    let fixture = Directory::new();
    let signal = Signal::default();
    let mut a = Node::new(&fixture.0.join("a"), signal.clone(), Some(3));
    let mut b = Node::new(&fixture.0.join("b"), signal, None);
    let Accepted::Connection(connection) = a
        .transport
        .driver
        .submit(Request::Connect(
            ConnectRequest::new(b.address, ExpectedPeer::Known(b.transport.local_id)).unwrap(),
        ))
        .unwrap()
    else {
        panic!("connection admission");
    };
    until(&mut a, &mut b, |a, b| {
        a.authenticated == 1 && b.authenticated == 1
    });
    for _ in 0..3 {
        a.bridge.open(&mut a.transport.driver, &connection).unwrap();
    }
    until(&mut a, &mut b, |a, b| {
        a.bridge.active() == 0 && b.bridge.active() == 0
    });
    let results = a.results.lock().unwrap();
    assert_eq!(
        results.completed.len(),
        2,
        "requester closures: {:?}; provider closures: {:?}",
        results.closed,
        b.results.lock().unwrap().closed
    );
    assert_eq!(
        results
            .completed
            .iter()
            .map(|(a, _)| a.operation.stream)
            .collect::<std::collections::BTreeSet<_>>(),
        [1, 5].into_iter().collect()
    );
    for (association, board) in &results.completed {
        assert_eq!(association.local, IdentityId(a.transport.local_id.0));
        assert_eq!(association.peer, IdentityId(b.transport.local_id.0));
        assert_eq!(
            board,
            &serde_json::json!({"version":1,"providedServices":[],"wantedServices":[]})
        );
    }
    assert!(results.closed.iter().any(|(id, outcome)| *id == 3
        && matches!(
            outcome,
            StreamOutcome::Failed(Failure {
                code: FailureCode::ProtocolViolation,
                ..
            })
        )));
    assert_eq!(a.transport.driver.resources().connections(), 1);
    assert_eq!(b.transport.driver.resources().connections(), 1);
}

struct Directory(std::path::PathBuf);
impl Directory {
    // Atomically create a private randomly named test root without stale PID-counter collisions.
    fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let mut random = [0; 16];
        getrandom::fill(&mut random).unwrap();
        let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let root = std::env::temp_dir().join(format!("talksphere-peer-wire-{suffix}"));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        Self(root)
    }
}
impl Drop for Directory {
    // Remove only this fixture's uniquely owned identity and runtime tree after nodes have dropped.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
