//! Control-driver admission and timer behavior with deterministic dependencies.
use super::*;
use crate::modules::transport::{
    adapters::TcpProvider,
    test_support::{
        clock::ManualClock, crypto::ScriptedCryptography, identity::ScriptedIdentity,
        script::Script,
    },
};
use std::time::Duration;

// Empty credential scripts prove listening and capacity rejection do not authenticate sockets.
#[test]
fn listener_control_and_registration_are_explicit() {
    let config = TransportConfig::new(
        super::super::config::ResourceLimits {
            connections: 1,
            streams_per_connection: 1,
            outbound_bytes: 65564,
            buffered_bytes_per_stream: 65536,
        },
        Duration::from_secs(1),
    )
    .unwrap();
    let mut driver = Driver::new(
        config,
        Dependencies {
            sockets: TcpProvider::new().unwrap(),
            clock: ManualClock::new(Instant::now()),
            identity: ScriptedIdentity(Script::new([])),
            cryptography: ScriptedCryptography(Script::new([])),
        },
    );
    let Accepted::Registration(_) = driver
        .submit(Request::RegisterProtocol {
            protocol: Protocol {
                id: "cr2se.peer".into(),
                version: 1,
            },
        })
        .unwrap()
    else {
        panic!("registration missing");
    };
    assert!(
        driver
            .submit(Request::RegisterProtocol {
                protocol: Protocol {
                    id: "cr2se.peer".into(),
                    version: 1
                }
            })
            .is_err()
    );
    let Accepted::Listener { handle, local_addr } = driver
        .submit(Request::Listen {
            endpoint: "127.0.0.1:0".parse().unwrap(),
        })
        .unwrap()
    else {
        panic!("listener missing");
    };
    assert_ne!(local_addr.port(), 0);
    assert!(matches!(
        driver.submit(Request::Listen {
            endpoint: "127.0.0.1:0".parse().unwrap()
        }),
        Err(TransportError::Backpressure)
    ));
    driver
        .submit(Request::StopListening {
            listener: handle.clone(),
        })
        .unwrap();
    assert!(
        matches!(driver.poll_event(&mut Context::from_waker(std::task::Waker::noop())), Poll::Ready(Some(Event::ListenerClosed { listener, failure: None })) if listener == handle)
    );
    assert!(matches!(
        driver.submit(Request::StopListening { listener: handle }),
        Err(TransportError::Closed)
    ));
    assert_eq!(driver.resources().connections(), 0);
}

// A full outgoing handshake preserves the admission handle and all local operations enforce capabilities.
#[test]
fn outgoing_control_contract_through_authentication_and_close() {
    use crate::modules::transport::test_support::{
        handshake,
        network::{NetworkCall, NetworkReply, ScriptedConnecting, ScriptedNetwork},
        socket::{ReadStep, ScriptedSocket, WriteStep},
    };
    let config = TransportConfig::new(
        super::super::config::ResourceLimits {
            connections: 1,
            streams_per_connection: 4,
            outbound_bytes: 4 * 65564,
            buffered_bytes_per_stream: 65536,
        },
        Duration::from_secs(1),
    )
    .unwrap();
    let endpoint = "127.0.0.1:4321".parse().unwrap();
    let (identity, cryptography, secrets, incoming) =
        handshake::handshake(ConnectionRole::Initiator);
    let (socket, control) = ScriptedSocket::new();
    control.push_read(ReadStep::Bytes(incoming));
    control.push_write(WriteStep::Accept(usize::MAX));
    control.push_write(WriteStep::Accept(usize::MAX));
    let (attempt, completion) = ScriptedConnecting::new();
    completion.push(Ok(Box::new(socket)));
    let network = ScriptedNetwork(Script::new([(
        NetworkCall::Connect(endpoint),
        NetworkReply::Connect(Ok(Box::new(attempt))),
    )]));
    let mut driver = Driver::new(
        config,
        Dependencies {
            sockets: network,
            clock: ManualClock::new(Instant::now()),
            identity,
            cryptography,
        },
    );
    let Accepted::Registration(owner) = driver
        .submit(Request::RegisterProtocol {
            protocol: Protocol {
                id: "cr2se.peer".into(),
                version: 1,
            },
        })
        .unwrap()
    else {
        panic!("missing owner");
    };
    let Accepted::Connection(connection) = driver
        .submit(Request::Connect(
            ConnectRequest::new(endpoint, ExpectedPeer::Unpinned).unwrap(),
        ))
        .unwrap()
    else {
        panic!("missing handle");
    };
    assert!(matches!(
        driver.submit(Request::Connect(
            ConnectRequest::new(endpoint, ExpectedPeer::Unpinned).unwrap()
        )),
        Err(TransportError::Backpressure)
    ));
    let mut cx = Context::from_waker(std::task::Waker::noop());
    assert!(
        matches!(driver.poll_event(&mut cx), Poll::Ready(Some(Event::Authenticated(context))) if context.connection == *connection.id())
    );
    let Accepted::Stream(stream) = driver
        .submit(Request::OpenStream {
            connection: connection.clone(),
            owner: owner.clone(),
        })
        .unwrap()
    else {
        panic!("missing stream");
    };
    assert!(matches!(
        driver.submit(Request::Stream {
            owner: super::super::handles::StreamOwner::new(),
            stream: stream.clone(),
            action: StreamAction::Data(vec![1])
        }),
        Err(TransportError::WrongStreamOwner)
    ));
    for action in [
        StreamAction::Data(vec![1]),
        StreamAction::End,
        StreamAction::Cancel(CancellationReason::Cancelled),
    ] {
        driver
            .submit(Request::Stream {
                owner: owner.clone(),
                stream: stream.clone(),
                action,
            })
            .unwrap();
    }
    driver
        .submit(Request::Ping {
            connection: connection.clone(),
            token: [1; 8],
            timeout: Duration::from_secs(1),
        })
        .unwrap();
    driver
        .submit(Request::Drain {
            connection: connection.clone(),
            reason: ShutdownReason::Normal,
        })
        .unwrap();
    driver
        .submit(Request::Close {
            connection: connection.clone(),
        })
        .unwrap();
    let mut closed = 0;
    for _ in 0..10 {
        if matches!(
            driver.poll_event(&mut cx),
            Poll::Ready(Some(Event::ConnectionClosed { .. }))
        ) {
            closed += 1;
        }
    }
    assert_eq!(closed, 1);
    assert!(control.is_closed());
    assert_eq!(driver.resources().connections(), 0);
    assert_eq!(driver.resources().outbound_bytes(), 0);
    assert!(matches!(
        driver.submit(Request::Close { connection }),
        Err(TransportError::WrongConnection)
    ));
    let mut released = secrets.released();
    released.sort_unstable();
    assert_eq!(released, vec![1, 2, 3, 4, 5]);
    driver.dependencies.identity.0.assert_finished();
    driver.dependencies.cryptography.0.assert_finished();
}

// Pending attempts are bounded, cancelled on explicit close and failed by deterministic deadlines.
#[test]
fn pending_connect_deadline_and_cancellation() {
    use crate::modules::transport::test_support::network::{
        NetworkCall, NetworkReply, ScriptedConnecting, ScriptedNetwork,
    };
    for cancel in [false, true] {
        let config = TransportConfig::new(
            super::super::config::ResourceLimits {
                connections: 1,
                streams_per_connection: 1,
                outbound_bytes: 65564,
                buffered_bytes_per_stream: 65536,
            },
            Duration::from_secs(1),
        )
        .unwrap();
        let clock = ManualClock::new(Instant::now());
        let endpoint = "127.0.0.1:4321".parse().unwrap();
        let (attempt, control) = ScriptedConnecting::new();
        let mut driver = Driver::new(
            config,
            Dependencies {
                sockets: ScriptedNetwork(Script::new([(
                    NetworkCall::Connect(endpoint),
                    NetworkReply::Connect(Ok(Box::new(attempt))),
                )])),
                clock: clock.clone(),
                identity: ScriptedIdentity(Script::new([])),
                cryptography: ScriptedCryptography(Script::new([])),
            },
        );
        let Accepted::Connection(connection) = driver
            .submit(Request::Connect(
                ConnectRequest::new(endpoint, ExpectedPeer::Unpinned).unwrap(),
            ))
            .unwrap()
        else {
            panic!("missing handle");
        };
        let mut cx = Context::from_waker(std::task::Waker::noop());
        assert!(driver.poll_event(&mut cx).is_pending());
        if cancel {
            driver.submit(Request::Close { connection }).unwrap();
        } else {
            clock.advance(Duration::from_secs(1)).unwrap();
        }
        assert!(matches!(
            driver.poll_event(&mut cx),
            Poll::Ready(Some(Event::ConnectionClosed { .. }))
        ));
        assert!(control.is_closed());
        assert_eq!(driver.resources().connections(), 0);
    }
}

// Incoming authority is announced before authentication, and a full driver does not consume another accept.
#[test]
fn incoming_admission_order_and_listener_failure() {
    use crate::modules::transport::test_support::{
        handshake,
        network::{NetworkCall, NetworkReply, ScriptedListener, ScriptedNetwork},
        socket::{ReadStep, ScriptedSocket, WriteStep},
    };
    let config = TransportConfig::new(
        super::super::config::ResourceLimits {
            connections: 1,
            streams_per_connection: 1,
            outbound_bytes: 65564,
            buffered_bytes_per_stream: 65536,
        },
        Duration::from_secs(1),
    )
    .unwrap();
    let endpoint = "127.0.0.1:4321".parse().unwrap();
    let (identity, cryptography, _, incoming) = handshake::handshake(ConnectionRole::Acceptor);
    let (socket, control) = ScriptedSocket::new();
    control.push_read(ReadStep::Bytes(incoming));
    control.push_write(WriteStep::Accept(usize::MAX));
    control.push_write(WriteStep::Accept(usize::MAX));
    let (listener, accepts) = ScriptedListener::new(endpoint);
    accepts.push(Ok(Box::new(socket)));
    accepts.push(Err(std::io::ErrorKind::ConnectionAborted.into()));
    let mut driver = Driver::new(
        config,
        Dependencies {
            sockets: ScriptedNetwork(Script::new([(
                NetworkCall::Listen(endpoint),
                NetworkReply::Listen(Ok(Box::new(listener))),
            )])),
            clock: ManualClock::new(Instant::now()),
            identity,
            cryptography,
        },
    );
    driver.submit(Request::Listen { endpoint }).unwrap();
    let mut cx = Context::from_waker(std::task::Waker::noop());
    let Poll::Ready(Some(Event::IncomingConnection(handle))) = driver.poll_event(&mut cx) else {
        panic!("authority must precede authentication");
    };
    assert!(matches!(
        driver.poll_event(&mut cx),
        Poll::Ready(Some(Event::Authenticated(_)))
    ));
    assert!(!accepts.is_closed());
    driver
        .submit(Request::Close { connection: handle })
        .unwrap();
    assert!(matches!(
        driver.poll_event(&mut cx),
        Poll::Ready(Some(Event::ConnectionClosed { .. }))
    ));
    for _ in 0..3 {
        if matches!(
            driver.poll_event(&mut cx),
            Poll::Ready(Some(Event::ListenerClosed {
                failure: Some(_),
                ..
            }))
        ) {
            assert!(accepts.is_closed());
            return;
        }
    }
    panic!("listener failure was not propagated after capacity recovered");
}
