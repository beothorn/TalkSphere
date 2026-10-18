//! Socket lifecycle tests using scripted dependencies and explicitly advanced time.
use super::*;
use crate::modules::transport::{
    config::{MAX_FRAME_BYTES, MAX_PAYLOAD_BYTES, ResourceLimits},
    ports::{
        crypto::EphemeralKey,
        identity::{IdentityId, PublicIdentity},
        io::Clock,
    },
    test_support::{
        clock::ManualClock,
        crypto::{CryptoCall, CryptoReply, ScriptedCryptography, SecretLog},
        identity::{IdentityCall, IdentityReply, ScriptedIdentity},
        script::Script,
        socket::{ReadStep, ScriptedSocket, SocketControl, WriteStep},
    },
};
use std::task::Waker;

// Minimal finite policy retains the full protocol frame size while bounding handshakes.
fn config() -> TransportConfig {
    TransportConfig::new(
        ResourceLimits {
            connections: 2,
            streams_per_connection: 8,
            outbound_bytes: 4 * MAX_FRAME_BYTES,
            buffered_bytes_per_stream: MAX_PAYLOAD_BYTES,
        },
        Duration::from_secs(3),
    )
    .unwrap()
}

// Start only the credential/entropy portion so no peer progress is assumed by cleanup tests.
fn pending(
    now: Instant,
) -> (
    Connection<ScriptedCryptography>,
    SocketControl,
    ScriptedIdentity,
    ScriptedCryptography,
    SecretLog,
) {
    let secrets = SecretLog::default();
    let identity = ScriptedIdentity(Script::new([(
        IdentityCall::PublicIdentity,
        IdentityReply::PublicIdentity(Ok(PublicIdentity {
            id: IdentityId([1; 32]),
            signing_public_key: [2; 32],
        })),
    )]));
    let mut crypto = ScriptedCryptography(Script::new([
        (
            CryptoCall::EphemeralKey,
            CryptoReply::EphemeralKey(Ok(EphemeralKey {
                public_key: [3; 32],
                private_key: secrets.issue(1),
            })),
        ),
        (CryptoCall::RandomNonce, CryptoReply::Bytes32(Ok([4; 32]))),
    ]));
    let (socket, control) = ScriptedSocket::new();
    let connection = Connection::begin(
        Box::new(socket),
        Options {
            role: ConnectionRole::Initiator,
            expected_peer: ExpectedPeer::Unpinned,
        },
        config(),
        now,
        &identity,
        &mut crypto,
    )
    .unwrap();
    (connection, control, identity, crypto, secrets)
}

// Time advances deterministically; deadline expiry closes a stalled socket and releases its secret once.
#[test]
fn stalled_handshake_deadline_releases_every_owned_resource() {
    let mut clock = ManualClock::new(Instant::now());
    let (mut connection, socket, mut identity, mut crypto, secrets) = pending(clock.now());
    let mut cx = Context::from_waker(Waker::noop());
    let handle = connection.handle().clone();
    assert!(
        connection
            .poll(
                &mut cx,
                clock.now(),
                &Registry::default(),
                &mut identity,
                &mut crypto
            )
            .is_pending()
    );
    assert!(connection.session().is_none());
    assert!(
        clock
            .poll_deadline(&mut cx, connection.next_deadline().unwrap())
            .is_pending()
    );
    clock.advance(Duration::from_secs(3)).unwrap();
    assert!(
        connection
            .poll(
                &mut cx,
                clock.now(),
                &Registry::default(),
                &mut identity,
                &mut crypto
            )
            .is_ready()
    );
    assert!(socket.is_closed());
    assert_eq!(secrets.released(), vec![1]);
    assert!(connection.next_deadline().is_none());
    assert!(
        matches!(connection.pop_event(), Some(Event::ConnectionClosed { connection, failure: Some(Failure { code: FailureCode::DeadlineExceeded, .. }) }) if connection == *handle.id())
    );
    connection.close();
    assert!(connection.pop_event().is_none());
}

// EOF, partial-frame EOF and write failure release the socket without reporting authentication.
#[test]
fn eof_truncation_write_failure_and_drop_cleanup() {
    for case in 0..4 {
        let now = Instant::now();
        let (mut connection, socket, mut identity, mut crypto, secrets) = pending(now);
        match case {
            0 => socket.push_read(ReadStep::Eof),
            1 => {
                socket.push_read(ReadStep::Bytes(vec![1, 1]));
                socket.push_read(ReadStep::Eof);
            }
            2 => socket.push_write(WriteStep::Error(std::io::ErrorKind::BrokenPipe)),
            _ => {}
        }
        if case < 3 {
            assert!(
                connection
                    .poll(
                        &mut Context::from_waker(Waker::noop()),
                        now,
                        &Registry::default(),
                        &mut identity,
                        &mut crypto
                    )
                    .is_ready()
            );
            assert!(matches!(
                connection.pop_event(),
                Some(Event::ConnectionClosed { .. })
            ));
        }
        drop(connection);
        assert!(socket.is_closed());
        assert_eq!(secrets.released(), vec![1]);
    }
}

// An unsupported version closes silently after HELLO; a safe malformed AUTH gets one bounded ERROR.
#[test]
fn safe_response_and_unsupported_version_silence() {
    for unsupported in [false, true] {
        let now = Instant::now();
        let (mut connection, socket, mut identity, mut crypto, secrets) = pending(now);
        socket.push_write(WriteStep::Accept(usize::MAX));
        let mut header = super::super::codec::FrameHeader::new(FrameType::Auth, 0, 0)
            .unwrap()
            .encode();
        if unsupported {
            header[0] = 2;
        }
        socket.push_read(ReadStep::Bytes(header.to_vec()));
        if !unsupported {
            socket.push_write(WriteStep::Accept(usize::MAX));
        }
        assert!(
            connection
                .poll(
                    &mut Context::from_waker(Waker::noop()),
                    now,
                    &Registry::default(),
                    &mut identity,
                    &mut crypto
                )
                .is_ready()
        );
        assert_eq!(socket.written().len(), if unsupported { 112 } else { 128 });
        assert!(socket.is_closed());
        assert_eq!(secrets.released(), vec![1]);
        assert!(matches!(
            connection.pop_event(),
            Some(Event::ConnectionClosed { .. })
        ));
        assert!(connection.pop_event().is_none());
    }
}

use crate::modules::transport::test_support::handshake as fixtures;

// Coalesced HELLO/AUTH crosses both phases and closure drops established keys and unfinished streams.
#[test]
fn authenticated_connection_preserves_handle_and_releases_keys() {
    use crate::modules::transport::{
        api::{Protocol, StreamOutcome},
        handshake::nonce,
    };
    for role in [ConnectionRole::Initiator, ConnectionRole::Acceptor] {
        let now = Instant::now();
        let (mut identity, mut crypto, secrets, incoming) = fixtures::handshake(role);
        let (socket, control) = ScriptedSocket::new();
        control.push_read(ReadStep::Bytes(incoming));
        control.push_write(WriteStep::Accept(usize::MAX));
        control.push_write(WriteStep::Accept(usize::MAX));
        let mut connection = Connection::begin(
            Box::new(socket),
            Options {
                role,
                expected_peer: ExpectedPeer::Unpinned,
            },
            config(),
            now,
            &identity,
            &mut crypto,
        )
        .unwrap();
        let handle = connection.handle().clone();
        let mut registry = Registry::default();
        let owner = registry
            .register(Protocol {
                id: "cr2se.peer".into(),
                version: 1,
            })
            .unwrap();
        let mut cx = Context::from_waker(Waker::noop());
        assert!(
            connection
                .poll(&mut cx, now, &registry, &mut identity, &mut crypto)
                .is_pending()
        );
        assert!(
            matches!(connection.pop_event(), Some(Event::Authenticated(context)) if context.connection == *handle.id())
        );
        let stream = connection
            .session()
            .unwrap()
            .open(&registry, &owner)
            .unwrap();
        assert_eq!(stream.connection_id(), handle.id());
        let id = stream.wire_id().get();
        let payload = super::super::codec::open::Open::new("cr2se.peer", 1)
            .unwrap()
            .encode();
        let header =
            super::super::codec::FrameHeader::new(FrameType::Open, id, payload.len() as u32)
                .unwrap();
        let (key, prefix) = if role == ConnectionRole::Initiator {
            (4, [6; 16])
        } else {
            (5, [7; 16])
        };
        crypto.0.push(
            CryptoCall::Tag {
                key,
                nonce: nonce(&prefix, 0),
                authenticated_data: [header.encode().to_vec(), payload].concat(),
            },
            CryptoReply::Bytes16(Ok([8; 16])),
        );
        control.push_write(WriteStep::Accept(usize::MAX));
        assert!(
            connection
                .poll(&mut cx, now, &registry, &mut identity, &mut crypto)
                .is_pending()
        );
        control.push_read(ReadStep::Eof);
        assert!(
            connection
                .poll(&mut cx, now, &registry, &mut identity, &mut crypto)
                .is_ready()
        );
        assert!(control.is_closed());
        let mut released = secrets.released();
        released.sort_unstable();
        assert_eq!(released, vec![1, 2, 3, 4, 5]);
        let mut failures = 0;
        while let Some(event) = connection.pop_event() {
            if matches!(
                event,
                Event::StreamClosed {
                    outcome: StreamOutcome::Failed(Failure {
                        code: FailureCode::ConnectionLost,
                        ..
                    }),
                    ..
                }
            ) {
                failures += 1;
            }
        }
        assert_eq!(failures, 1);
        identity.0.assert_finished();
        crypto.0.assert_finished();
    }
}

// Shared admission refuses a socket before any credential call, then recovers after reservation release.
#[test]
fn connection_limit_precedes_authentication() {
    use crate::modules::transport::resources::Resources;
    let resources = Resources::new(*config().limits());
    let first = resources.connection().unwrap();
    let second = resources.connection().unwrap();
    let (socket, control) = ScriptedSocket::new();
    let identity = ScriptedIdentity(Script::new([]));
    let mut crypto = ScriptedCryptography(Script::new([]));
    let result = Connection::begin_shared(
        Box::new(socket),
        Options {
            role: ConnectionRole::Acceptor,
            expected_peer: ExpectedPeer::Unpinned,
        },
        config(),
        Instant::now(),
        &identity,
        &mut crypto,
        resources.clone(),
    );
    assert_eq!(
        result.err().unwrap().failure.code,
        FailureCode::ResourceLimit
    );
    assert!(control.is_closed());
    assert_eq!(resources.connections(), 2);
    drop(first);
    drop(second);
    assert_eq!(resources.connections(), 0);
    assert_eq!(resources.outbound_bytes(), 0);
    assert!(identity.0.calls().is_empty());
    assert!(crypto.0.calls().is_empty());
}

#[path = "tests/acceptance.rs"]
mod acceptance;
