//! Composed TR-01–TR-15 scenarios with exact scripted tag requests, never simulated security claims.
use super::*;
use crate::modules::transport::{
    api::{Protocol, ShutdownReason, StreamOutcome},
    codec::{
        FrameHeader,
        control::{ErrorPayload, GoAway},
        open::Open,
    },
    handles::StreamOwner,
    handshake::nonce,
    ports::DependencyError,
};

struct Harness {
    connection: Connection<ScriptedCryptography>,
    control: SocketControl,
    identity: ScriptedIdentity,
    crypto: ScriptedCryptography,
    secrets: SecretLog,
    registry: Registry,
    owner: StreamOwner,
    now: Instant,
    receive_sequence: u64,
    send_sequence: u64,
}
impl Harness {
    // Establish the upstream fixture through real framing/state composition, then retain scripts for probes.
    fn new() -> Self {
        let (identity, mut crypto, secrets, incoming) =
            fixtures::handshake(ConnectionRole::Initiator);
        let now = Instant::now();
        let (socket, control) = ScriptedSocket::new();
        control.push_read(ReadStep::Bytes(incoming));
        for _ in 0..2 {
            control.push_write(WriteStep::Accept(usize::MAX));
        }
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
        let mut registry = Registry::default();
        let owner = registry
            .register(Protocol {
                id: "cr2se.peer".into(),
                version: 1,
            })
            .unwrap();
        let mut harness = Self {
            connection,
            control,
            identity,
            crypto,
            secrets,
            registry,
            owner,
            now,
            receive_sequence: 0,
            send_sequence: 0,
        };
        assert!(harness.poll().is_pending());
        assert!(matches!(
            harness.connection.pop_event(),
            Some(Event::Authenticated(_))
        ));
        harness
    }
    // Drive one bounded slice without advancing injected time.
    fn poll(&mut self) -> Poll<()> {
        self.connection.poll(
            &mut Context::from_waker(Waker::noop()),
            self.now,
            &self.registry,
            &mut self.identity,
            &mut self.crypto,
        )
    }
    // Script the exact AAD, directional key and sequence for one incoming protected frame.
    fn expect_receive(&mut self, kind: FrameType, id: u32, payload: &[u8], valid: bool) -> Vec<u8> {
        let aad = aad(kind, id, payload);
        self.crypto.0.push(
            CryptoCall::VerifyTag {
                key: 5,
                nonce: nonce(&[7; 16], self.receive_sequence),
                authenticated_data: aad.clone(),
                tag: [8; 16],
            },
            CryptoReply::Verified(if valid {
                Ok(())
            } else {
                Err(DependencyError::VerificationFailed)
            }),
        );
        if valid {
            self.receive_sequence += 1;
        }
        [aad, vec![8; 16]].concat()
    }
    // Script an outgoing tag and allow a complete write; returned bytes are an independent oracle.
    fn expect_send(&mut self, kind: FrameType, id: u32, payload: &[u8]) -> Vec<u8> {
        let aad = aad(kind, id, payload);
        self.crypto.0.push(
            CryptoCall::Tag {
                key: 4,
                nonce: nonce(&[6; 16], self.send_sequence),
                authenticated_data: aad.clone(),
            },
            CryptoReply::Bytes16(Ok([9; 16])),
        );
        self.send_sequence += 1;
        self.control.push_write(WriteStep::Accept(usize::MAX));
        [aad, vec![9; 16]].concat()
    }
    // Receive one authenticated frame and optionally check the exact encoded response.
    fn receive(
        &mut self,
        kind: FrameType,
        id: u32,
        payload: Vec<u8>,
        response: Option<(FrameType, u32, Vec<u8>)>,
    ) {
        let bytes = self.expect_receive(kind, id, &payload, true);
        let start = self.control.written().len();
        let expected = response
            .map(|(kind, id, payload)| self.expect_send(kind, id, &payload))
            .unwrap_or_default();
        self.control.push_read(ReadStep::Bytes(bytes));
        let _ = self.poll();
        assert_eq!(&self.control.written()[start..], expected);
        self.crypto.0.assert_finished();
    }
    // Transfer all currently queued application observations for assertions.
    fn events(&mut self) -> Vec<Event> {
        std::iter::from_fn(|| self.connection.pop_event()).collect()
    }
}
// Canonical independent wire header plus payload is authenticated, with no payload interpretation.
fn aad(kind: FrameType, id: u32, payload: &[u8]) -> Vec<u8> {
    [
        FrameHeader::new(kind, id, payload.len() as u32)
            .unwrap()
            .encode()
            .to_vec(),
        payload.to_vec(),
    ]
    .concat()
}
// Diagnostic-free errors make response scope and exact registry code unambiguous.
fn error(code: ErrorCode) -> Vec<u8> {
    ErrorPayload::new(code, "").unwrap().encode()
}

// TR-02: every split across OPEN plus DATA, including coalesced bytes, delivers the same ordered payload.
#[test]
fn every_protected_split_and_coalesced_frames() {
    let open = Open::new("cr2se.peer", 1).unwrap().encode();
    let length = open.len() + 28 + 3 + 28;
    for split in 0..=length {
        let mut h = Harness::new();
        let mut bytes = h.expect_receive(FrameType::Open, 2, &open, true);
        bytes.extend(h.expect_receive(FrameType::Data, 2, b"abc", true));
        for part in [&bytes[..split], &bytes[split..]] {
            if !part.is_empty() {
                h.control.push_read(ReadStep::Bytes(part.to_vec()));
            }
        }
        assert!(h.poll().is_pending());
        let events = h.events();
        assert!(matches!(&events[0], Event::StreamOpened { .. }));
        assert!(matches!(&events[1], Event::Data { bytes, .. } if bytes == b"abc"));
        h.crypto.0.assert_finished();
    }
}

// TR-08/09/10: refusal consumes IDs, half-closing permits opposite traffic and terminal frames are ignored.
#[test]
fn routing_halves_cancellation_and_reuse_through_socket() {
    let mut h = Harness::new();
    h.receive(
        FrameType::Open,
        2,
        vec![],
        Some((FrameType::Error, 2, error(ErrorCode::ProtocolError))),
    );
    h.receive(
        FrameType::Open,
        4,
        Open::new("unknown", 1).unwrap().encode(),
        Some((FrameType::Error, 4, error(ErrorCode::UnsupportedProtocol))),
    );
    h.receive(
        FrameType::Open,
        6,
        Open::new("cr2se.peer", 1).unwrap().encode(),
        None,
    );
    let stream = match h.events().pop().unwrap() {
        Event::StreamOpened { stream, .. } => stream,
        event => panic!("unexpected {event:?}"),
    };
    h.receive(FrameType::Data, 6, vec![1; MAX_PAYLOAD_BYTES], None);
    assert!(
        matches!(h.events().pop(), Some(Event::Data { bytes, .. }) if bytes.len() == MAX_PAYLOAD_BYTES)
    );
    h.receive(FrameType::End, 6, vec![], None);
    h.connection
        .session()
        .unwrap()
        .send(&h.owner, &stream, b"reply".to_vec())
        .unwrap();
    h.expect_send(FrameType::Data, 6, b"reply");
    assert!(h.poll().is_pending());
    h.connection
        .session()
        .unwrap()
        .end(&h.owner, &stream)
        .unwrap();
    h.expect_send(FrameType::End, 6, &[]);
    assert!(h.poll().is_pending());
    assert!(h.events().iter().any(|e| matches!(
        e,
        Event::StreamClosed {
            outcome: StreamOutcome::Completed,
            ..
        }
    )));
    h.receive(
        FrameType::Open,
        8,
        Open::new("cr2se.peer", 1).unwrap().encode(),
        None,
    );
    h.receive(FrameType::Cancel, 8, error(ErrorCode::Cancelled), None);
    h.receive(FrameType::Data, 8, vec![1], None);
    h.receive(FrameType::End, 8, vec![99], None);
    assert_eq!(
        h.events()
            .iter()
            .filter(|e| matches!(e, Event::StreamClosed { .. }))
            .count(),
        1
    );
    h.receive(
        FrameType::Open,
        8,
        vec![],
        Some((FrameType::Error, 0, error(ErrorCode::ProtocolError))),
    );
    assert!(h.control.is_closed());
}

// TR-03: invalid preliminary headers close without dispatch; version mismatch cannot send an ERROR.
#[test]
fn malformed_header_response_matrix() {
    for case in 0..6 {
        let mut h = Harness::new();
        let mut header = FrameHeader::new(FrameType::Data, 2, 1).unwrap().encode();
        match case {
            0 => header[0] = 2,
            1 => header[2] = 1,
            2 => header[1] = 255,
            3 => header[8..12].copy_from_slice(&65537u32.to_be_bytes()),
            4 => header[4..8].fill(0),
            _ => header[1] = FrameType::Ping as u8,
        }
        let start = h.control.written().len();
        if case != 0 {
            h.expect_send(
                FrameType::Error,
                0,
                &error(if case == 3 {
                    ErrorCode::FrameSizeError
                } else {
                    ErrorCode::ProtocolError
                }),
            );
        }
        h.control.push_read(ReadStep::Bytes(header.to_vec()));
        assert!(h.poll().is_ready());
        assert!(h.control.is_closed());
        if case == 0 {
            assert_eq!(h.control.written().len(), start);
        }
        assert!(!h.events().iter().any(|e| matches!(e, Event::Data { .. })));
        h.crypto.0.assert_finished();
    }
}

// TR-06: verifier rejection dispatches nothing, emits no response and releases all directional secrets.
#[test]
fn failed_integrity_closes_silently() {
    let mut h = Harness::new();
    let bytes = h.expect_receive(
        FrameType::Open,
        2,
        &Open::new("cr2se.peer", 1).unwrap().encode(),
        false,
    );
    let start = h.control.written().len();
    h.control.push_read(ReadStep::Bytes(bytes));
    assert!(h.poll().is_ready());
    assert_eq!(h.control.written().len(), start);
    assert!(h.events().iter().all(|e| matches!(
        e,
        Event::ConnectionClosed {
            failure: Some(Failure {
                code: FailureCode::IntegrityFailed,
                ..
            }),
            ..
        }
    )));
    let mut released = h.secrets.released();
    released.sort_unstable();
    assert_eq!(released, vec![1, 2, 3, 4, 5]);
}

// TR-11/12: exact echo and ignored PONGs coexist with graceful closure after complete GOAWAY output.
#[test]
fn ping_matching_timeout_and_graceful_empty_drain() {
    let mut h = Harness::new();
    h.receive(
        FrameType::Ping,
        0,
        vec![1; 8],
        Some((FrameType::Pong, 0, vec![1; 8])),
    );
    h.receive(FrameType::Pong, 0, vec![2; 8], None);
    h.connection
        .session()
        .unwrap()
        .ping([3; 8], Duration::from_secs(1), h.now)
        .unwrap();
    h.expect_send(FrameType::Ping, 0, &[3; 8]);
    assert!(h.poll().is_pending());
    h.receive(FrameType::Pong, 0, vec![4; 8], None);
    h.now += Duration::from_secs(1);
    assert!(h.poll().is_pending());
    assert!(h.events().iter().any(|e| matches!(
        e,
        Event::PingFinished {
            result: Err(Failure {
                code: FailureCode::DeadlineExceeded,
                ..
            }),
            ..
        }
    )));
    h.connection
        .session()
        .unwrap()
        .drain(ShutdownReason::Normal)
        .unwrap();
    h.expect_send(
        FrameType::GoAway,
        0,
        &GoAway::new(0, ErrorCode::NoError, "").unwrap().encode(),
    );
    assert!(h.poll().is_ready());
    assert!(
        h.events()
            .iter()
            .any(|e| matches!(e, Event::ConnectionClosed { failure: None, .. }))
    );
}

// TR-13: malformed protected controls pass integrity before failing their actual payload rule.
#[test]
fn malformed_protected_controls_are_connection_errors() {
    for (kind, payload) in [
        (FrameType::Ping, vec![0; 7]),
        (FrameType::Error, vec![0, 0, 0, 0]),
        (FrameType::Error, vec![255, 255, 0, 0]),
        (FrameType::Error, vec![0, 1, 0, 1, 255]),
        (FrameType::GoAway, vec![0; 7]),
    ] {
        let mut h = Harness::new();
        h.receive(
            kind,
            0,
            payload,
            Some((FrameType::Error, 0, error(ErrorCode::ProtocolError))),
        );
        assert!(h.control.is_closed());
    }
}

// TR-14: every partial DATA prefix fails unfinished streams without provisional payload delivery.
#[test]
fn every_incomplete_protected_prefix_is_truncation() {
    let bytes = [aad(FrameType::Data, 2, b"abc"), vec![8; 16]].concat();
    for end in 1..bytes.len() {
        let mut h = Harness::new();
        h.receive(
            FrameType::Open,
            2,
            Open::new("cr2se.peer", 1).unwrap().encode(),
            None,
        );
        h.events();
        h.control.push_read(ReadStep::Bytes(bytes[..end].to_vec()));
        h.control.push_read(ReadStep::Eof);
        assert!(h.poll().is_ready());
        let events = h.events();
        assert!(!events.iter().any(|e| matches!(e, Event::Data { .. })));
        assert!(events.iter().any(|e| matches!(
            e,
            Event::StreamClosed {
                outcome: StreamOutcome::Failed(Failure {
                    code: FailureCode::TruncatedFrame,
                    ..
                }),
                ..
            }
        )));
    }
}

// TR-01/02/05: byte-at-a-time handshake plus protected frames survive short writes and the tag-phase transition.
#[test]
fn byte_segmented_handshake_and_protected_tail_with_short_writes() {
    use crate::modules::transport::test_support::vectors;
    let now = Instant::now();
    let (mut identity, mut crypto, _, mut incoming) =
        fixtures::handshake(ConnectionRole::Initiator);
    let open = Open::new("cr2se.peer", 1).unwrap().encode();
    for (sequence, kind, payload) in [
        (0, FrameType::Open, open),
        (1, FrameType::Data, b"abc".to_vec()),
    ] {
        let authenticated_data = aad(kind, 2, &payload);
        crypto.0.push(
            CryptoCall::VerifyTag {
                key: 5,
                nonce: nonce(&[7; 16], sequence),
                authenticated_data: authenticated_data.clone(),
                tag: [8; 16],
            },
            CryptoReply::Verified(Ok(())),
        );
        incoming.extend(authenticated_data);
        incoming.extend([8; 16]);
    }
    let (socket, control) = ScriptedSocket::new();
    for byte in incoming {
        control.push_read(ReadStep::Bytes(vec![byte]));
    }
    for index in 0..188 {
        control.push_write(WriteStep::Accept(index % 3 + 1));
    }
    let mut connection = Connection::begin(
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
    let mut registry = Registry::default();
    registry
        .register(Protocol {
            id: "cr2se.peer".into(),
            version: 1,
        })
        .unwrap();
    for _ in 0..10 {
        let _ = connection.poll(
            &mut Context::from_waker(Waker::noop()),
            now,
            &registry,
            &mut identity,
            &mut crypto,
        );
    }
    let events: Vec<_> = std::iter::from_fn(|| connection.pop_event()).collect();
    assert!(matches!(&events[0], Event::Authenticated(_)));
    assert!(matches!(&events[1], Event::StreamOpened { .. }));
    assert!(matches!(&events[2], Event::Data { bytes, .. } if bytes == b"abc"));
    let expected = [
        aad(FrameType::Hello, 0, &vectors::bytes("initiator HELLO", 0)),
        aad(
            FrameType::Auth,
            0,
            &vectors::bytes("initiator AUTH payload", 0),
        ),
    ]
    .concat();
    assert_eq!(control.written(), expected);
    crypto.0.assert_finished();
    identity.0.assert_finished();
}

// TR-12: validated GOAWAY refuses higher local streams and late OPENs while accepted work completes normally.
#[test]
fn goaway_refusal_and_accepted_stream_completion() {
    let mut h = Harness::new();
    let mut streams = vec![];
    for id in [1, 3, 5] {
        streams.push(
            h.connection
                .session()
                .unwrap()
                .open(&h.registry, &h.owner)
                .unwrap(),
        );
        h.expect_send(
            FrameType::Open,
            id,
            &Open::new("cr2se.peer", 1).unwrap().encode(),
        );
    }
    assert!(h.poll().is_pending());
    h.events();
    h.receive(
        FrameType::GoAway,
        0,
        GoAway::new(3, ErrorCode::NoError, "").unwrap().encode(),
        Some((
            FrameType::GoAway,
            0,
            GoAway::new(0, ErrorCode::NoError, "").unwrap().encode(),
        )),
    );
    assert!(h.events().iter().any(|e| matches!(e, Event::StreamClosed { stream, outcome: StreamOutcome::Failed(Failure { code: FailureCode::Refused, .. }) } if stream.wire_id().get() == 5)));
    h.receive(
        FrameType::Open,
        2,
        vec![],
        Some((FrameType::Error, 2, error(ErrorCode::RefusedStream))),
    );
    h.receive(
        FrameType::GoAway,
        0,
        GoAway::new(1, ErrorCode::NoError, "").unwrap().encode(),
        None,
    );
    h.events();
    h.receive(FrameType::End, 1, vec![], None);
    h.connection
        .session()
        .unwrap()
        .end(&h.owner, &streams[0])
        .unwrap();
    h.expect_send(FrameType::End, 1, &[]);
    assert!(h.poll().is_ready());
    assert!(h.events().iter().any(|e| matches!(
        e,
        Event::StreamClosed {
            outcome: StreamOutcome::Completed,
            ..
        }
    )));
}

// TR-07/12: never-created IDs and invalid drain bounds fail the whole connection after successful integrity.
#[test]
fn protected_state_violations_close_connection() {
    for (kind, id, payload) in [
        (
            FrameType::Open,
            4,
            Open::new("cr2se.peer", 1).unwrap().encode(),
        ),
        (FrameType::Data, 2, vec![1]),
        (
            FrameType::GoAway,
            0,
            GoAway::new(2, ErrorCode::NoError, "").unwrap().encode(),
        ),
        (
            FrameType::GoAway,
            0,
            GoAway::new(1, ErrorCode::NoError, "").unwrap().encode(),
        ),
    ] {
        let mut h = Harness::new();
        h.receive(
            kind,
            id,
            payload,
            Some((FrameType::Error, 0, error(ErrorCode::ProtocolError))),
        );
        assert!(h.control.is_closed());
    }
}

// TR-15: overflowing one consumer fails that stream while a second still receives verified bytes.
#[test]
fn bounded_slow_consumer_does_not_stop_another_stream() {
    let mut h = Harness::new();
    for id in [2, 4] {
        h.receive(
            FrameType::Open,
            id,
            Open::new("cr2se.peer", 1).unwrap().encode(),
            None,
        );
    }
    h.events();
    h.receive(FrameType::Data, 2, vec![0; MAX_PAYLOAD_BYTES], None);
    h.receive(
        FrameType::Data,
        2,
        vec![1],
        Some((FrameType::Error, 2, error(ErrorCode::ResourceLimit))),
    );
    h.receive(FrameType::Data, 4, vec![9], None);
    let events = h.events();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::StreamClosed {
            outcome: StreamOutcome::Failed(Failure {
                code: FailureCode::ResourceLimit,
                ..
            }),
            ..
        }
    )));
    assert!(events.iter().any(|e| matches!(e, Event::Data { stream, bytes } if stream.wire_id().get() == 4 && *bytes == [9])));
    assert!(!h.control.is_closed());
}

// A peer may close TCP immediately after the drain is complete; EOF must preserve normal completion.
#[test]
fn eof_after_completed_goaway_is_normal() {
    let mut h = Harness::new();
    h.connection
        .session()
        .unwrap()
        .drain(ShutdownReason::Normal)
        .unwrap();
    h.expect_send(
        FrameType::GoAway,
        0,
        &GoAway::new(0, ErrorCode::NoError, "").unwrap().encode(),
    );
    h.control.push_read(ReadStep::Eof);
    assert!(h.poll().is_ready());
    assert!(
        h.events()
            .iter()
            .any(|e| matches!(e, Event::ConnectionClosed { failure: None, .. }))
    );
}

// Nonzero peer GOAWAY reasons survive graceful transport drain as typed connection outcomes.
#[test]
fn peer_drain_reason_is_preserved() {
    let mut h = Harness::new();
    h.receive(
        FrameType::GoAway,
        0,
        GoAway::new(0, ErrorCode::IdExhausted, "").unwrap().encode(),
        Some((
            FrameType::GoAway,
            0,
            GoAway::new(0, ErrorCode::NoError, "").unwrap().encode(),
        )),
    );
    assert!(h.control.is_closed());
    assert!(h.events().iter().any(|e| matches!(
        e,
        Event::ConnectionClosed {
            failure: Some(Failure {
                origin: FailureOrigin::Peer,
                code: FailureCode::IdExhausted
            }),
            ..
        }
    )));
}
