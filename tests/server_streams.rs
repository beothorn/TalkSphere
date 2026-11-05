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
        assert!(
            Command::new(env!("CARGO_BIN_EXE_talksphere"))
                .args(["identity-init", "--identity-store"])
                .arg(path)
                .output()
                .unwrap()
                .status
                .success()
        );
        let config = Config::new(Options::local(path.into()).unwrap()).unwrap();
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
        assert!(!remaining.is_zero(), "real TCP condition timed out");
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
