//! Trusted-boundary adapter tests; these injected observations do not prove network authentication.
use super::*;
use crate::modules::transport::{api::ConnectionRole, handles::ConnectionHandle};
use std::{
    num::NonZeroU32,
    sync::{Mutex, mpsc},
    task::{Wake, Waker},
    thread::ThreadId,
};

#[derive(Default)]
struct Seen {
    opens: Vec<Association>,
    records: Vec<(u32, Record)>,
    ends: Vec<u32>,
    closed: Vec<(u32, StreamOutcome)>,
    threads: Vec<ThreadId>,
}
type BlockGate = (mpsc::Sender<()>, Mutex<Option<mpsc::Receiver<()>>>);
struct Probe {
    seen: Arc<Mutex<Seen>>,
    block: Option<BlockGate>,
}
impl Factory for Probe {
    // Record only verified association and thread metadata; local requests are explicit fixture policy.
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
        let mut seen = self.seen.lock().unwrap();
        seen.opens.push(association);
        seen.threads.push(std::thread::current().id());
        if opener == Direction::Local {
            output.enqueue(&Record::Control(br#"{"type":"board.get"}"#.to_vec()))?;
            output.end();
        }
        let block = if association.operation.stream == 1 {
            self.block.as_ref().and_then(|(entered, gate)| {
                gate.lock()
                    .unwrap()
                    .take()
                    .map(|gate| (entered.clone(), gate))
            })
        } else {
            None
        };
        Ok(Box::new(ProbeHandler {
            stream: association.operation.stream,
            seen: self.seen.clone(),
            block,
            local: opener == Direction::Local,
        }))
    }
}
struct ProbeHandler {
    stream: u32,
    seen: Arc<Mutex<Seen>>,
    block: Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>,
    local: bool,
}
impl Handler for ProbeHandler {
    // Echo complete records for framing assertions; no service, payment or conformance claim is made.
    fn record(&mut self, record: Record, output: &mut Output) -> Result<(), Error> {
        if let Some((entered, gate)) = self.block.take() {
            entered.send(()).unwrap();
            gate.recv().unwrap();
        }
        self.seen
            .lock()
            .unwrap()
            .records
            .push((self.stream, record.clone()));
        if !self.local {
            output.enqueue(&record)?;
        }
        Ok(())
    }
    // Record a complete decoded direction and request END for remotely opened fixture streams.
    fn remote_end(&mut self, output: &mut Output) -> Result<(), Error> {
        self.seen.lock().unwrap().ends.push(self.stream);
        if !self.local {
            output.end();
        }
        Ok(())
    }
    // This fixed echo has no incremental producer; the adapter still supports drain notifications.
    fn writable(&mut self, _: &mut Output) -> Result<(), Error> {
        Ok(())
    }
    // Retain actual closure observations so tests can distinguish normal completion from uncertainty.
    fn closed(&mut self, outcome: StreamOutcome) -> Result<(), Error> {
        self.seen
            .lock()
            .unwrap()
            .closed
            .push((self.stream, outcome));
        Ok(())
    }
}
#[derive(Debug, PartialEq, Eq)]
enum Sent {
    Data(Vec<u8>),
    End,
    Protocol,
    Cancel(CancellationReason),
}
struct Control {
    owner: StreamOwner,
    sent: Vec<(u32, Sent)>,
    blocked: bool,
    reject_registration: bool,
    next: u32,
}
impl TransportControl for Control {
    // Model only local admission, preserving exact byte copies and ownership for adapter assertions.
    fn submit(&mut self, request: Request) -> Result<Accepted, TransportError> {
        match request {
            Request::RegisterProtocol { protocol: selected }
                if selected == protocol() && !self.reject_registration =>
            {
                Ok(Accepted::Registration(self.owner.clone()))
            }
            Request::OpenStream { connection, owner } if owner == self.owner => {
                let stream =
                    StreamHandle::new(&connection, &owner, NonZeroU32::new(self.next).unwrap());
                self.next += 2;
                Ok(Accepted::Stream(stream))
            }
            Request::Stream {
                owner,
                stream,
                action,
            } => {
                stream.authorize(stream.connection_id(), &owner)?;
                let sent = match action {
                    StreamAction::Data(bytes) if !self.blocked => Sent::Data(bytes),
                    StreamAction::End if !self.blocked => Sent::End,
                    StreamAction::Data(_) | StreamAction::End => {
                        return Err(TransportError::Backpressure);
                    }
                    StreamAction::ProtocolError => Sent::Protocol,
                    StreamAction::Cancel(reason) => Sent::Cancel(reason),
                };
                self.sent.push((stream.wire_id().get(), sent));
                Ok(Accepted::Queued)
            }
            _ => Err(TransportError::InvalidRequest),
        }
    }
    // Unit tests explicitly inject trusted observations; no socket handshake is simulated here.
    fn poll_event(&mut self, _: &mut Context<'_>) -> Poll<Option<Event>> {
        Poll::Pending
    }
}
// Return small finite defaults with room for independent maximum-sized control records.
fn config() -> Config {
    Config {
        connections: 4,
        exchanges: 4,
        workers: 2,
        input_bytes: 131_072,
        output_bytes: CONTROL_MAX + 5,
        memory_bytes: 32_000_000,
        controls_per_second: 32,
        lifetime: Duration::from_secs(60),
    }
}
// Create only trusted local fixture authority, with no network or identity conformance claim.
fn setup(
    block: Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>,
) -> (
    Bridge,
    Control,
    Arc<Mutex<Seen>>,
    ConnectionHandle,
    PeerContext,
) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let factory = Arc::new(Probe {
        seen: seen.clone(),
        block: block.map(|(entered, gate)| (entered, Mutex::new(Some(gate)))),
    });
    let mut control = Control {
        owner: StreamOwner::new(),
        sent: Vec::new(),
        blocked: false,
        reject_registration: false,
        next: 1,
    };
    let mut bridge = Bridge::register(&mut control, config(), factory).unwrap();
    let connection = ConnectionHandle::new();
    let context = PeerContext {
        connection: connection.id().clone(),
        role: ConnectionRole::Acceptor,
        local_identity: crate::modules::transport::ports::identity::IdentityId([2; 32]),
        peer_identity: crate::modules::transport::ports::identity::IdentityId([1; 32]),
    };
    bridge
        .event(&mut control, Event::Authenticated(context.clone()))
        .unwrap();
    (bridge, control, seen, connection, context)
}
// Inject a remotely opened stream carrying the same owner and authenticated instance observation.
fn incoming(
    bridge: &mut Bridge,
    control: &mut Control,
    connection: &ConnectionHandle,
    context: &PeerContext,
    id: u32,
) -> StreamHandle {
    let stream = StreamHandle::new(connection, &control.owner, NonZeroU32::new(id).unwrap());
    bridge
        .event(
            control,
            Event::StreamOpened {
                context: context.clone(),
                stream: stream.clone(),
                protocol: protocol(),
            },
        )
        .unwrap();
    stream
}
struct Notify(mpsc::Sender<()>);
impl Wake for Notify {
    // Wake a finite test observer when a real worker reply becomes available.
    fn wake(self: Arc<Self>) {
        let _ = self.0.send(());
    }
}
// Drive until an observable predicate holds, waiting on actual worker wakeups rather than sleeping.
fn drive(bridge: &mut Bridge, control: &mut Control, done: impl Fn(&Bridge, &Control) -> bool) {
    let (sender, receiver) = mpsc::channel();
    let waker = Waker::from(Arc::new(Notify(sender)));
    let mut context = Context::from_waker(&waker);
    loop {
        bridge.poll(control, &mut context).unwrap();
        if done(bridge, control) {
            break;
        }
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("adapter progress wake");
    }
}
// Fragmentation and output admission preserve exact bytes, and a malformed sibling stays isolated.
#[test]
fn framing_and_protocol_failure_are_stream_scoped() {
    let (mut bridge, mut control, seen, connection, context) = setup(None);
    let bad = incoming(&mut bridge, &mut control, &connection, &context, 1);
    let good = incoming(&mut bridge, &mut control, &connection, &context, 3);
    bridge
        .event(
            &mut control,
            Event::Data {
                stream: bad,
                bytes: vec![9, 0, 0, 0, 1, 0],
            },
        )
        .unwrap();
    let record = Record::Control(br#"{"type":"board.get"}"#.to_vec());
    let wire = record.encode().unwrap();
    for bytes in wire.chunks(3) {
        bridge
            .event(
                &mut control,
                Event::Data {
                    stream: good.clone(),
                    bytes: bytes.to_vec(),
                },
            )
            .unwrap();
    }
    bridge
        .event(
            &mut control,
            Event::RemoteEnd {
                stream: good.clone(),
            },
        )
        .unwrap();
    drive(&mut bridge, &mut control, |_, c| {
        c.sent.contains(&(1, Sent::Protocol)) && c.sent.contains(&(3, Sent::End))
    });
    let received: Vec<u8> = control
        .sent
        .iter()
        .filter_map(|(id, sent)| match sent {
            Sent::Data(bytes) if *id == 3 => Some(bytes.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(received, wire);
    assert_eq!(seen.lock().unwrap().records, vec![(3, record)]);
    assert!(
        seen.lock()
            .unwrap()
            .threads
            .iter()
            .all(|id| *id != std::thread::current().id())
    );
}
// Backpressure retains complete records and END; a later reactor turn can progress without a new event.
#[test]
fn backpressure_does_not_lose_bytes_or_wait_for_an_application_event() {
    let (mut bridge, mut control, _, connection, context) = setup(None);
    control.blocked = true;
    let stream = bridge.open(&mut control, &connection).unwrap();
    bridge
        .event(
            &mut control,
            Event::StreamOpened {
                context,
                stream,
                protocol: protocol(),
            },
        )
        .unwrap();
    drive(&mut bridge, &mut control, |b, _| {
        b.entries[0]
            .state
            .as_ref()
            .is_some_and(|s| s.output.queue.pending() > 0)
    });
    assert!(control.sent.is_empty());
    control.blocked = false;
    let (notify, notified) = mpsc::channel();
    let waker = Waker::from(Arc::new(Notify(notify)));
    bridge
        .poll(&mut control, &mut Context::from_waker(&waker))
        .unwrap();
    assert!(
        notified.try_recv().is_ok(),
        "new Transport writes need another reactor turn"
    );
    assert!(matches!(
        control.sent.as_slice(),
        [(1, Sent::Data(_)), (1, Sent::End)]
    ));
}
// A normal close can be queued before workers process received bytes; those bytes and END stay ordered.
#[test]
fn normal_closure_waits_for_received_records_and_remote_end() {
    let (mut bridge, mut control, seen, connection, _) = setup(None);
    let stream = bridge.open(&mut control, &connection).unwrap();
    drive(&mut bridge, &mut control, |_, c| {
        c.sent.contains(&(1, Sent::End))
    });
    let response = Record::Control(br#"{"type":"board","board":{}}"#.to_vec());
    bridge
        .event(
            &mut control,
            Event::Data {
                stream: stream.clone(),
                bytes: response.encode().unwrap(),
            },
        )
        .unwrap();
    bridge
        .event(
            &mut control,
            Event::RemoteEnd {
                stream: stream.clone(),
            },
        )
        .unwrap();
    bridge
        .event(
            &mut control,
            Event::StreamClosed {
                stream,
                outcome: StreamOutcome::Completed,
            },
        )
        .unwrap();
    drive(&mut bridge, &mut control, |b, _| b.active() == 0);
    let seen = seen.lock().unwrap();
    assert_eq!(seen.records, vec![(1, response)]);
    assert_eq!(seen.ends, vec![1]);
    assert_eq!(seen.closed, vec![(1, StreamOutcome::Completed)]);
}
// In-flight bytes stay charged, a blocked callback leaves its sibling usable, and late closure reaches its owner.
#[test]
fn blocked_work_preserves_bounds_and_late_closure_without_stalling_siblings() {
    let (entered, entry) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (mut bridge, mut control, seen, connection, context) = setup(Some((entered, gate)));
    let first = incoming(&mut bridge, &mut control, &connection, &context, 1);
    let second = incoming(&mut bridge, &mut control, &connection, &context, 3);
    let wire = Record::Control(vec![b'x'; 32]).encode().unwrap();
    bridge.config.input_bytes = wire.len();
    bridge
        .event(
            &mut control,
            Event::Data {
                stream: first.clone(),
                bytes: wire.clone(),
            },
        )
        .unwrap();
    drive(&mut bridge, &mut control, |b, _| {
        b.entries[0].flight_bytes == wire.len()
    });
    entry.recv_timeout(Duration::from_secs(5)).unwrap();
    bridge
        .event(
            &mut control,
            Event::Data {
                stream: first.clone(),
                bytes: vec![1],
            },
        )
        .unwrap();
    assert!(
        control
            .sent
            .contains(&(1, Sent::Cancel(CancellationReason::ResourceLimit)))
    );
    bridge
        .event(
            &mut control,
            Event::Data {
                stream: second.clone(),
                bytes: wire,
            },
        )
        .unwrap();
    bridge
        .event(&mut control, Event::RemoteEnd { stream: second })
        .unwrap();
    drive(&mut bridge, &mut control, |_, c| {
        c.sent.contains(&(3, Sent::End))
    });
    let outcome = StreamOutcome::Cancelled {
        origin: crate::modules::transport::api::FailureOrigin::Local,
        reason: CancellationReason::ResourceLimit,
    };
    bridge
        .event(
            &mut control,
            Event::StreamClosed {
                stream: first,
                outcome,
            },
        )
        .unwrap();
    release.send(()).unwrap();
    drive(&mut bridge, &mut control, |b, _| b.active() == 1);
    assert!(seen.lock().unwrap().closed.contains(&(1, outcome)));
    assert!(
        !control
            .sent
            .iter()
            .any(|(id, action)| *id == 1 && matches!(action, Sent::Data(_)))
    );
}
// Reconnecting with the same peer and wire number produces another operation identity without nonce reuse.
#[test]
fn connection_instances_and_authenticated_contexts_are_independent() {
    let (mut bridge, mut control, seen, connection, context) = setup(None);
    incoming(&mut bridge, &mut control, &connection, &context, 1);
    drive(&mut bridge, &mut control, |_, _| {
        seen.lock().unwrap().opens.len() == 1
    });
    bridge
        .event(
            &mut control,
            Event::ConnectionClosed {
                connection: connection.id().clone(),
                failure: None,
            },
        )
        .unwrap();
    drive(&mut bridge, &mut control, |b, _| b.active() == 0);
    let new = ConnectionHandle::new();
    let mut fresh = context.clone();
    fresh.connection = new.id().clone();
    bridge
        .event(&mut control, Event::Authenticated(fresh.clone()))
        .unwrap();
    incoming(&mut bridge, &mut control, &new, &fresh, 1);
    drive(&mut bridge, &mut control, |_, _| {
        seen.lock().unwrap().opens.len() == 2
    });
    let seen = seen.lock().unwrap();
    assert_ne!(seen.opens[0].operation, seen.opens[1].operation);
    assert_eq!(seen.opens[0].peer, seen.opens[1].peer);
}
// Admission is checked before per-stream allocation and after stop; unverified contexts never invoke a factory.
#[test]
fn memory_authority_deadline_and_stop_are_enforced() {
    let (mut bridge, mut control, seen, connection, context) = setup(None);
    bridge.config.memory_bytes = bridge.reservation;
    let first = incoming(&mut bridge, &mut control, &connection, &context, 1);
    incoming(&mut bridge, &mut control, &connection, &context, 3);
    assert_eq!(bridge.active(), 1);
    assert!(
        control
            .sent
            .contains(&(3, Sent::Cancel(CancellationReason::ResourceLimit)))
    );
    drive(&mut bridge, &mut control, |_, _| {
        seen.lock().unwrap().opens.len() == 1
    });
    bridge.entries[0].deadline = Instant::now();
    bridge
        .poll(&mut control, &mut Context::from_waker(Waker::noop()))
        .unwrap();
    assert!(
        control
            .sent
            .contains(&(1, Sent::Cancel(CancellationReason::Cancelled)))
    );
    bridge
        .event(
            &mut control,
            Event::StreamClosed {
                stream: first,
                outcome: StreamOutcome::Completed,
            },
        )
        .unwrap();
    drive(&mut bridge, &mut control, |b, _| b.active() == 0);
    let mut forged = context.clone();
    forged.peer_identity.0 = [8; 32];
    incoming(&mut bridge, &mut control, &connection, &forged, 5);
    assert_eq!(bridge.active(), 0);
    bridge.stop();
    assert_eq!(bridge.open(&mut control, &connection), Err(Error::Limit));
}
// A partial record at END and a control flood fail only their own stream with distinct failure classes.
#[test]
fn truncation_and_control_rate_are_bounded() {
    let (mut bridge, mut control, _, connection, context) = setup(None);
    bridge.config.controls_per_second = 1;
    let truncated = incoming(&mut bridge, &mut control, &connection, &context, 1);
    let flooded = incoming(&mut bridge, &mut control, &connection, &context, 3);
    bridge
        .event(
            &mut control,
            Event::Data {
                stream: truncated.clone(),
                bytes: vec![1, 0],
            },
        )
        .unwrap();
    bridge
        .event(&mut control, Event::RemoteEnd { stream: truncated })
        .unwrap();
    let record = Record::Control(vec![b'x']).encode().unwrap();
    bridge
        .event(
            &mut control,
            Event::Data {
                stream: flooded,
                bytes: [record.clone(), record].concat(),
            },
        )
        .unwrap();
    drive(&mut bridge, &mut control, |_, c| {
        c.sent.contains(&(1, Sent::Protocol))
            && c.sent
                .contains(&(3, Sent::Cancel(CancellationReason::ResourceLimit)))
    });
}

// Real Driver registration and opaque stream admission remain separate from authenticated network conformance.
#[test]
fn actual_transport_driver_requires_authentication_before_open() {
    use crate::modules::transport::{
        api::{ConnectRequest, ExpectedPeer},
        config::{MAX_FRAME_BYTES, MAX_PAYLOAD_BYTES, ResourceLimits, TransportConfig},
        driver::Driver,
        ports::Dependencies,
        test_support::{
            clock::ManualClock,
            handshake,
            network::{NetworkCall, NetworkReply, ScriptedConnecting, ScriptedNetwork},
            script::Script,
            socket::{ReadStep, ScriptedSocket, WriteStep},
        },
    };
    let endpoint = "127.0.0.1:4321".parse().unwrap();
    let (identity, cryptography, _, incoming) = handshake::handshake(ConnectionRole::Initiator);
    let (socket, control) = ScriptedSocket::new();
    control.push_read(ReadStep::Bytes(incoming));
    control.push_write(WriteStep::Accept(usize::MAX));
    control.push_write(WriteStep::Accept(usize::MAX));
    let (attempt, completion) = ScriptedConnecting::new();
    completion.push(Ok(Box::new(socket)));
    let config = TransportConfig::new(
        ResourceLimits {
            connections: 1,
            streams_per_connection: 4,
            outbound_bytes: 4 * MAX_FRAME_BYTES,
            buffered_bytes_per_stream: MAX_PAYLOAD_BYTES,
        },
        Duration::from_secs(1),
    )
    .unwrap();
    let mut driver = Driver::new(
        config,
        Dependencies {
            sockets: ScriptedNetwork(Script::new([(
                NetworkCall::Connect(endpoint),
                NetworkReply::Connect(Ok(Box::new(attempt))),
            )])),
            clock: ManualClock::new(Instant::now()),
            identity,
            cryptography,
        },
    );
    let factory = Arc::new(Probe {
        seen: Arc::new(Mutex::new(Seen::default())),
        block: None,
    });
    let mut bridge =
        Bridge::register(&mut driver, super::tests::config(), factory.clone()).unwrap();
    assert!(matches!(
        Bridge::register(&mut driver, super::tests::config(), factory),
        Err(Error::Unavailable)
    ));
    let Accepted::Connection(connection) = driver
        .submit(Request::Connect(
            ConnectRequest::new(endpoint, ExpectedPeer::Unpinned).unwrap(),
        ))
        .unwrap()
    else {
        panic!("connection admission");
    };
    assert_eq!(bridge.open(&mut driver, &connection), Err(Error::Denied));
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(Some(event @ Event::Authenticated(_))) = driver.poll_event(&mut context) else {
        panic!("authenticated driver event");
    };
    bridge.event(&mut driver, event).unwrap();
    assert!(bridge.open(&mut driver, &connection).is_ok());
    driver.submit(Request::Close { connection }).unwrap();
    for _ in 0..8 {
        if let Poll::Ready(Some(event)) = driver.poll_event(&mut context) {
            bridge.event(&mut driver, event).unwrap();
        }
    }
    bridge.poll(&mut driver, &mut context).unwrap();
    assert!(control.is_closed());
}
// Invalid byte arithmetic, impossible worker counts and post-END writes refuse before effects.
#[test]
fn invalid_configuration_and_terminal_output_are_explicit() {
    for bad in [
        Config {
            workers: 0,
            ..config()
        },
        Config {
            input_bytes: usize::MAX,
            ..config()
        },
        Config {
            output_bytes: CONTROL_MAX,
            ..config()
        },
        Config {
            memory_bytes: 1,
            ..config()
        },
        Config {
            lifetime: Duration::ZERO,
            ..config()
        },
    ] {
        assert_eq!(bad.reservation(), Err(Error::Invalid));
    }
    let mut output = Output {
        queue: Outbound::new(16).unwrap(),
        end: false,
    };
    assert_eq!(output.available(), 16);
    let record = Record::Control(vec![b'x']);
    output.enqueue(&record).unwrap();
    assert_eq!(output.available(), 10);
    output.end();
    assert_eq!(output.enqueue(&record), Err(Error::Conflict));
}

// Requester roles follow selected work, including wanted reversal, rather than TCP or stream opener roles.
#[test]
fn scope_binds_only_authenticated_endpoints_without_guessing_requester_roles() {
    let scope = Scope {
        local: IdentityId([1; 32]),
        peer: IdentityId([2; 32]),
        connection: [3; 32],
        stream: 7,
        deadline: Instant::now() + Duration::from_secs(60),
    };
    let provided = scope.association(scope.peer).unwrap();
    let wanted = scope.association(scope.local).unwrap();
    assert_eq!(provided.operation.connection, wanted.operation.connection);
    assert_eq!(provided.operation.stream, wanted.operation.stream);
    assert_ne!(provided.operation.requester, wanted.operation.requester);
    assert_eq!(scope.association(IdentityId([4; 32])), Err(Error::Denied));
}
// Unrelated protocol ownership is returned to composition, and exhausted counters never wrap into old instances.
#[test]
fn foreign_events_and_exhausted_instances_never_reach_handlers() {
    let (mut bridge, mut control, seen, connection, context) = setup(None);
    let foreign = StreamHandle::new(
        &connection,
        &StreamOwner::new(),
        NonZeroU32::new(1).unwrap(),
    );
    assert!(matches!(
        bridge
            .event(
                &mut control,
                Event::Data {
                    stream: foreign,
                    bytes: vec![1]
                }
            )
            .unwrap(),
        Some(Event::Data { .. })
    ));
    bridge.stream_sequence = u64::MAX;
    assert_eq!(bridge.open(&mut control, &connection), Err(Error::Limit));
    bridge.connection_sequence = u64::MAX;
    let fresh = PeerContext {
        connection: ConnectionHandle::new().id().clone(),
        ..context
    };
    assert!(matches!(
        bridge.event(&mut control, Event::Authenticated(fresh)),
        Err(Error::Limit)
    ));
    assert!(seen.lock().unwrap().opens.is_empty());
    assert_eq!(bridge.active(), 0);
}

// Queue delay never restarts the original deadline, and closure cleanup remains permitted after expiry.
#[test]
fn expired_worker_input_never_starts_a_fresh_handler() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let factory = Arc::new(Probe {
        seen: seen.clone(),
        block: None,
    });
    let scope = Scope {
        local: IdentityId([1; 32]),
        peer: IdentityId([2; 32]),
        connection: [3; 32],
        stream: 1,
        deadline: Instant::now(),
    };
    let state = State::new(factory, scope, Direction::Remote, config()).unwrap();
    let returned = state.run(1, Input::Open);
    assert_eq!(returned.result, Err(Error::Cancelled));
    assert!(seen.lock().unwrap().opens.is_empty());
    let state = returned.state.unwrap();
    let returned = state.run(
        1,
        Input::Closed(StreamOutcome::Cancelled {
            origin: crate::modules::transport::api::FailureOrigin::Local,
            reason: CancellationReason::Cancelled,
        }),
    );
    assert!(returned.cleanup);
    assert!(returned.state.is_none());
    assert_eq!(returned.result, Ok(()));
}
