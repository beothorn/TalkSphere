//! Actual TCP drivers with real protected identities and bounded test-only application consumers.
#![cfg(target_os = "linux")]
#[path = "support/runtime.rs"]
mod runtime;
use std::{
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use talksphere::{
    composition::server::{
        self, Failure,
        config::{Config, Options},
        pump::{Pump, Signal},
        routing::{Consumer, Router},
        transport::Transport,
    },
    modules::transport::{
        api::*,
        handles::{ConnectionHandle, StreamHandle, StreamOwner},
    },
};
struct Sink(Arc<Mutex<Vec<Event>>>);
impl Consumer for Sink {
    // Retain only a finite number of test observations; a runaway peer cannot create an unbounded test queue.
    fn event(&mut self, event: Event) -> Result<(), Failure> {
        let mut events = self.0.lock().unwrap();
        if events.len() >= 128 {
            return Err(Failure::Runtime);
        }
        events.push(event);
        Ok(())
    }
}
struct Node {
    pump: Pump<Transport>,
    router: Router,
    owner: StreamOwner,
    events: Arc<Mutex<Vec<Event>>>,
    address: std::net::SocketAddr,
}
impl Node {
    // Initialize separate protected credentials and register only an isolated test application protocol.
    fn new(path: &Path, signal: Signal) -> Self {
        Self::configured(path, signal, |_| {})
    }
    // Validate test-specific finite resource policy before acquiring real driver dependencies.
    fn configured(path: &Path, signal: Signal, edit: impl FnOnce(&mut Options)) -> Self {
        assert!(
            Command::new(env!("CARGO_BIN_EXE_talksphere"))
                .args(["identity-init", "--identity-store"])
                .arg(path)
                .output()
                .unwrap()
                .status
                .success()
        );
        let mut options = Options::local(path.into()).unwrap();
        edit(&mut options);
        let config = Config::new(options).unwrap();
        let credentials = server::credentials::load(&config, false).unwrap();
        let mut node = server::transport::compose(config, credentials).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut router = Router::default();
        let owner = router
            .register(
                &mut node.driver,
                Protocol {
                    id: "org.example.transport-test".into(),
                    version: 1,
                },
                Box::new(Sink(events.clone())),
            )
            .unwrap();
        let address = node.listen().unwrap().address;
        Self {
            pump: Pump::new(node, signal),
            router,
            owner,
            events,
            address,
        }
    }
    // Poll the real driver and route verified stream observations through actual application ownership checks.
    fn poll(&mut self) {
        let router = &mut self.router;
        let events = self.events.clone();
        self.pump
            .turn_with(|node, event| {
                node.observe(&event)?;
                if let Some(event) = router.route(&mut node.driver, event)? {
                    let mut events = events.lock().unwrap();
                    assert!(events.len() < 128);
                    events.push(event);
                }
                Ok(())
            })
            .unwrap();
    }
    // Inspect one exact protocol stream handle already issued to this node's consumer.
    fn stream(&self, id: u32) -> Option<StreamHandle> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .find_map(|event| match event {
                Event::StreamOpened { stream, .. } if stream.wire_id().get() == id => {
                    Some(stream.clone())
                }
                _ => None,
            })
    }
    // Read the exact concatenated verified bytes for one stream, independent of TCP read boundaries.
    fn bytes(&self, id: u32) -> Vec<u8> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match event {
                Event::Data { stream, bytes } if stream.wire_id().get() == id => {
                    Some(bytes.as_slice())
                }
                _ => None,
            })
            .flatten()
            .copied()
            .collect()
    }
    // Submit one stream action under this node's locally registered authority.
    fn action(&mut self, stream: &StreamHandle, action: StreamAction) {
        self.pump
            .transport
            .submit(Request::Stream {
                owner: self.owner.clone(),
                stream: stream.clone(),
                action,
            })
            .unwrap();
    }
    // Find a locally issued incoming connection handle for reverse-direction stream creation.
    fn incoming(&self) -> Option<ConnectionHandle> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .find_map(|event| match event {
                Event::IncomingConnection(handle) => Some(handle.clone()),
                _ => None,
            })
    }
}
// Drive both actual TCP endpoints until an observable condition, using shared readiness wakeups and an absolute timeout.
fn until(a: &mut Node, b: &mut Node, predicate: impl Fn(&Node, &Node) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        a.poll();
        b.poll();
        if predicate(a, b) {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "real TCP condition timed out: A {:?} connections={} events={:?}; B {:?} connections={} events={:?}",
            a.pump.transport.stage,
            a.pump.transport.driver.resources().connections(),
            summarize(a),
            b.pump.transport.stage,
            b.pump.transport.driver.resources().connections(),
            summarize(b)
        );
        a.pump.signal.wait(remaining).unwrap();
    }
}
// Establish a pinned connection and require authenticated events at both ends before application requests.
fn connect(a: &mut Node, b: &mut Node) -> ConnectionHandle {
    let Accepted::Connection(handle) = a
        .pump
        .transport
        .submit(Request::Connect(
            ConnectRequest::new(b.address, ExpectedPeer::Known(b.pump.transport.local_id)).unwrap(),
        ))
        .unwrap()
    else {
        panic!("connection admission missing")
    };
    until(a, b, |a, b| {
        [a, b].iter().all(|node| {
            node.events
                .lock()
                .unwrap()
                .iter()
                .any(|event| matches!(event, Event::Authenticated(_)))
        })
    });
    handle
}
// Open only an explicitly registered test protocol and retain the immediate local stream capability.
fn open(node: &mut Node, connection: &ConnectionHandle) -> StreamHandle {
    let Accepted::Stream(stream) = node
        .pump
        .transport
        .submit(Request::OpenStream {
            connection: connection.clone(),
            owner: node.owner.clone(),
        })
        .unwrap()
    else {
        panic!("stream admission missing")
    };
    stream
}

// Actual protected TCP multiplexing preserves both directions and independent halves while cancellation leaves another stream usable.
#[test]
fn real_tcp_multiplexing_halves_and_cancellation() {
    let fixture = runtime::ProcessFixture::spawn(&["--version"]);
    assert!(fixture.line().is_ok());
    let signal = Signal::default();
    let mut a = Node::new(&fixture.root.join("a"), signal.clone());
    let mut b = Node::new(&fixture.root.join("b"), signal);
    let connection = connect(&mut a, &mut b);
    let first = open(&mut a, &connection);
    let second = open(&mut a, &connection);
    let third = open(&mut a, &connection);
    assert_eq!(
        [
            first.wire_id().get(),
            second.wire_id().get(),
            third.wire_id().get()
        ],
        [1, 3, 5]
    );
    until(&mut a, &mut b, |_, b| b.stream(5).is_some());
    let remote = b.stream(1).unwrap();
    a.action(&first, StreamAction::Data(b"left".to_vec()));
    a.action(&second, StreamAction::Data(b"other".to_vec()));
    a.action(&first, StreamAction::End);
    until(&mut a, &mut b, |_, b| {
        b.bytes(1) == b"left" && b.bytes(3) == b"other"
    });
    b.action(&remote, StreamAction::Data(b"reply".to_vec()));
    b.action(&remote, StreamAction::End);
    until(&mut a, &mut b, |a, b| {
        a.bytes(1)==b"reply"&&[a,b].iter().all(|node|node.events.lock().unwrap().iter().any(|event|matches!(event,Event::StreamClosed{stream,outcome:StreamOutcome::Completed} if stream.wire_id().get()==1)))
    });
    a.action(&second, StreamAction::Cancel(CancellationReason::Cancelled));
    a.action(&third, StreamAction::Data(b"still usable".to_vec()));
    until(&mut a, &mut b, |_, b| {
        b.bytes(5) == b"still usable"
            && b.events.lock().unwrap().iter().any(
                |event| matches!(event,Event::StreamClosed{stream,..} if stream.wire_id().get()==3),
            )
    });
    let incoming = b.incoming().unwrap();
    let reverse = open(&mut b, &incoming);
    let reverse_second = open(&mut b, &incoming);
    let reverse_third = open(&mut b, &incoming);
    assert_eq!(
        [
            reverse.wire_id().get(),
            reverse_second.wire_id().get(),
            reverse_third.wire_id().get()
        ],
        [2, 4, 6]
    );
    until(&mut a, &mut b, |a, _| a.stream(2).is_some());
    let remote = a.stream(2).unwrap();
    b.action(&reverse, StreamAction::End);
    a.action(&remote, StreamAction::Data(b"opposite half".to_vec()));
    a.action(&remote, StreamAction::End);
    until(&mut a, &mut b, |_, b| b.bytes(2) == b"opposite half");
    let invalid = runtime::ProcessFixture::spawn(&["--unknown"]);
    assert!(invalid.error().is_ok());
}

// Actual TCP peers echo both probe tokens and finish accepted work during GOAWAY without accepting new streams.
#[test]
fn real_tcp_probes_and_accepted_stream_drain() {
    let fixture = runtime::ProcessFixture::spawn(&["--version"]);
    assert!(fixture.line().is_ok());
    let signal = Signal::default();
    let mut a = Node::new(&fixture.root.join("a"), signal.clone());
    let mut b = Node::new(&fixture.root.join("b"), signal);
    let connection = connect(&mut a, &mut b);
    let incoming = b.incoming().unwrap();
    a.pump
        .transport
        .submit(Request::Ping {
            connection: connection.clone(),
            token: [1; 8],
            timeout: Duration::from_secs(1),
        })
        .unwrap();
    b.pump
        .transport
        .submit(Request::Ping {
            connection: incoming,
            token: [2; 8],
            timeout: Duration::from_secs(1),
        })
        .unwrap();
    until(&mut a, &mut b, |a, b| {
        [a, b].iter().all(|node| {
            node.events
                .lock()
                .unwrap()
                .iter()
                .any(|event| matches!(event, Event::PingFinished { result: Ok(_), .. }))
        })
    });
    let stream = open(&mut a, &connection);
    until(&mut a, &mut b, |_, b| b.stream(1).is_some());
    let remote = b.stream(1).unwrap();
    a.pump.transport.begin_shutdown(Instant::now()).unwrap();
    assert!(
        a.pump
            .transport
            .submit(Request::OpenStream {
                connection,
                owner: a.owner.clone()
            })
            .is_err()
    );
    b.action(
        &remote,
        StreamAction::Data(b"accepted drain reply".to_vec()),
    );
    b.action(&remote, StreamAction::End);
    a.action(&stream, StreamAction::End);
    until(&mut a, &mut b, |a, b| {
        a.bytes(1) == b"accepted drain reply"
            && a.pump.transport.driver.resources().connections() == 0
            && b.pump.transport.driver.resources().connections() == 0
    });
    assert!(
        a.pump
            .transport
            .shutdown_progress(Instant::now(), false)
            .unwrap()
    );
    for node in [&a, &b] {
        assert!(node.events.lock().unwrap().iter().any(|event| matches!(
            event,
            Event::StreamClosed {
                outcome: StreamOutcome::Completed,
                ..
            }
        )));
    }
    let invalid = runtime::ProcessFixture::spawn(&["--unknown"]);
    assert!(invalid.error().is_ok());
}

// Describe lifecycle categories and stream IDs without dumping application bytes into failure diagnostics.
fn summarize(node: &Node) -> Vec<String> {
    node.events
        .lock()
        .unwrap()
        .iter()
        .map(|event| match event {
            Event::Authenticated(_) => "authenticated".into(),
            Event::IncomingConnection(_) => "incoming".into(),
            Event::StreamOpened { stream, .. } => format!("open {}", stream.wire_id()),
            Event::Data { stream, bytes } => {
                format!("data {} length {}", stream.wire_id(), bytes.len())
            }
            Event::RemoteEnd { stream } => format!("end {}", stream.wire_id()),
            Event::StreamClosed { stream, outcome } => {
                format!("closed {} {outcome:?}", stream.wire_id())
            }
            Event::PingFinished { token, result, .. } => format!("ping {} {result:?}", token[0]),
            Event::ConnectionClosed { failure, .. } => format!("connection closed {failure:?}"),
            Event::ListenerClosed { .. } => "listener closed".into(),
        })
        .collect()
}

// Real TCP driver admission remains bounded while a full-size stream and a small independent stream both progress.
#[test]
fn real_tcp_resource_admission_and_frame_fairness() {
    use talksphere::modules::transport::config::{MAX_PAYLOAD_BYTES, TransportConfig};
    let fixture = runtime::ProcessFixture::spawn(&["--version"]);
    assert!(fixture.line().is_ok());
    let signal = Signal::default();
    let limit = |options: &mut Options| {
        let mut limits = *options.transport.limits();
        limits.connections = 1;
        limits.streams_per_connection = 2;
        options.transport = TransportConfig::new(limits, Duration::from_secs(1)).unwrap();
    };
    let mut a = Node::configured(&fixture.root.join("a"), signal.clone(), limit);
    let mut b = Node::configured(&fixture.root.join("b"), signal, limit);
    let connection = connect(&mut a, &mut b);
    assert!(
        a.pump
            .transport
            .submit(Request::Connect(
                ConnectRequest::new(b.address, ExpectedPeer::Known(b.pump.transport.local_id))
                    .unwrap()
            ))
            .is_err()
    );
    assert_eq!(a.pump.transport.driver.resources().connections(), 1);
    let large = open(&mut a, &connection);
    let small = open(&mut a, &connection);
    assert!(
        a.pump
            .transport
            .submit(Request::OpenStream {
                connection,
                owner: a.owner.clone()
            })
            .is_err()
    );
    until(&mut a, &mut b, |_, b| b.stream(3).is_some());
    a.action(&large, StreamAction::Data(vec![9; MAX_PAYLOAD_BYTES]));
    a.action(&small, StreamAction::Data(vec![7]));
    assert!(
        a.pump.transport.driver.resources().outbound_bytes()
            <= a.pump
                .transport
                .config
                .options()
                .transport
                .limits()
                .outbound_bytes
    );
    until(&mut a, &mut b, |_, b| {
        b.bytes(1).len() == MAX_PAYLOAD_BYTES && b.bytes(3) == [7]
    });
    let invalid = runtime::ProcessFixture::spawn(&["--unknown"]);
    assert!(invalid.error().is_ok());
}
