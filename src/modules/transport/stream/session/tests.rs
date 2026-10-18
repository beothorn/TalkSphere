//! Consumer routing and authority scenarios; integrity is tested at its own boundary.
use super::*;
use crate::modules::transport::stream::registry::RegistrationError;

// Supply authenticated facts as a deterministic orchestration fixture.
fn setup() -> (Session, Registry, StreamOwner) {
    let mut registry = Registry::default();
    let owner = registry
        .register(Protocol {
            id: "cr2se.peer".into(),
            version: 1,
        })
        .unwrap();
    let session = Session::new(
        ConnectionRole::Initiator,
        IdentityId([1; 32]),
        IdentityId([2; 32]),
    );
    (session, registry, owner)
}

// Registrations validate grammar, reject duplicates and never accept unrelated authority.
#[test]
fn registry_authority_and_duplicates() {
    let (mut state, mut registry, owner) = setup();
    assert_eq!(
        registry.register(Protocol {
            id: "cr2se.peer".into(),
            version: 1
        }),
        Err(RegistrationError::Duplicate)
    );
    assert!(matches!(
        registry.register(Protocol {
            id: "Bad".into(),
            version: 1
        }),
        Err(RegistrationError::Invalid(_))
    ));
    let foreign = StreamOwner::new();
    assert_eq!(state.open(&registry, &foreign), Err(SessionError::Owner));
    let stream = state.open(&registry, &owner).unwrap();
    assert_eq!(stream.wire_id().get(), 1);
    assert_eq!(stream.authorize(state.handle().id(), &owner), Ok(()));
    let frame = state.pop_outbound().unwrap();
    assert_eq!(frame.header().frame_type(), FrameType::Open);
    assert_eq!(Open::decode(frame.payload()).unwrap().id(), "cr2se.peer");
    assert!(state.pop_outbound().is_none());
}

// Incoming selections queue authenticated context and preserve registration ownership.
#[test]
fn registered_open_queues_consumer_context() {
    let (mut state, registry, owner) = setup();
    assert!(matches!(state.pop_event(), Some(Event::Authenticated(_))));
    state
        .receive_parts(
            FrameType::Open,
            2,
            Open::new("cr2se.peer", 1).unwrap().encode(),
            &registry,
        )
        .unwrap();
    match state.pop_event().unwrap() {
        Event::StreamOpened {
            context,
            stream,
            protocol,
        } => {
            assert_eq!(context.peer_identity, IdentityId([2; 32]));
            assert_eq!(stream.authorize(state.handle().id(), &owner), Ok(()));
            assert_eq!(protocol.version, 1);
        }
        event => panic!("unexpected event {event:?}"),
    }
    assert!(state.pop_event().is_none());
}

// Malformed and unsupported OPENs consume their IDs while independent streams survive.
#[test]
fn refused_ids_are_consumed_and_others_remain_usable() {
    let (mut state, registry, owner) = setup();
    let existing = state.open(&registry, &owner).unwrap();
    state.pop_outbound();
    for (id, payload, code) in [
        (2, vec![], ErrorCode::ProtocolError),
        (
            4,
            Open::new("unknown", 1).unwrap().encode(),
            ErrorCode::UnsupportedProtocol,
        ),
        (
            6,
            Open::new("cr2se.peer", 2).unwrap().encode(),
            ErrorCode::UnsupportedProtocol,
        ),
    ] {
        state
            .receive_parts(FrameType::Open, id, payload, &registry)
            .unwrap();
        let error = state.pop_outbound().unwrap();
        assert_eq!(error.header().stream_id(), id);
        assert_eq!(ErrorPayload::decode(error.payload()).unwrap().code(), code);
        assert!(state.ids.was_created(id));
        assert!(!state.streams.contains_key(&id));
    }
    assert!(state.streams.contains_key(&existing.wire_id().get()));
    assert_eq!(
        state.receive_parts(FrameType::Open, 2, vec![], &registry),
        Err(SessionError::Id(IdError::Unexpected))
    );
    assert_eq!(
        state.receive_parts(FrameType::Data, 99, vec![1], &registry),
        Err(SessionError::Protocol)
    );
}

// Immediate peer DATA follows OPEN without acknowledgement; interleaving preserves each byte sequence.
#[test]
fn multiplexed_bidirectional_bytes_preserve_order() {
    let (mut state, registry, owner) = setup();
    let local = state.open(&registry, &owner).unwrap();
    state
        .receive_parts(
            FrameType::Open,
            2,
            Open::new("cr2se.peer", 1).unwrap().encode(),
            &registry,
        )
        .unwrap();
    let remote = state.streams[&2].handle.clone();
    for (stream, bytes) in [
        (&local, b"a".to_vec()),
        (&remote, b"xyz".to_vec()),
        (&local, b"bc".to_vec()),
    ] {
        state.send(&owner, stream, bytes.clone()).unwrap();
        state
            .receive_parts(FrameType::Data, stream.wire_id().get(), bytes, &registry)
            .unwrap();
    }
    let mut received = BTreeMap::<u32, Vec<u8>>::new();
    while let Some(event) = state.pop_event() {
        if let Event::Data { stream, bytes } = event {
            received
                .entry(stream.wire_id().get())
                .or_default()
                .extend(bytes);
        }
    }
    assert_eq!(received[&1], b"abc");
    assert_eq!(received[&2], b"xyz");
    let mut sent = BTreeMap::<u32, Vec<u8>>::new();
    while let Some(frame) = state.pop_outbound() {
        if frame.header().frame_type() == FrameType::Data {
            sent.entry(frame.header().stream_id())
                .or_default()
                .extend(frame.into_payload());
        }
    }
    assert_eq!(sent, received);
}

// Full payloads work; empty/oversized local sends and unauthorized callers queue nothing.
#[test]
fn data_bounds_authority_and_backpressure() {
    let (mut state, registry, owner) = setup();
    let stream = state.open(&registry, &owner).unwrap();
    state.pop_outbound();
    assert_eq!(
        state.send(&owner, &stream, vec![]),
        Err(SessionError::InvalidData)
    );
    assert_eq!(
        state.send(&owner, &stream, vec![0; MAX_PAYLOAD_BYTES + 1]),
        Err(SessionError::InvalidData)
    );
    assert!(matches!(
        state.send(&StreamOwner::new(), &stream, vec![1]),
        Err(SessionError::Authority(_))
    ));
    for _ in 0..4 {
        state
            .send(&owner, &stream, vec![1; MAX_PAYLOAD_BYTES])
            .unwrap();
    }
    assert_eq!(
        state.send(&owner, &stream, vec![1]),
        Err(SessionError::Capacity)
    );
    state.pop_outbound();
    state.send(&owner, &stream, vec![2]).unwrap();
    state
        .receive_parts(FrameType::Data, 1, vec![3; MAX_PAYLOAD_BYTES], &registry)
        .unwrap();
    assert!(
        state
            .events
            .iter()
            .any(|e| matches!(e, Event::Data { bytes, .. } if bytes.len() == MAX_PAYLOAD_BYTES))
    );
    state
        .receive_parts(FrameType::Data, 1, vec![3], &registry)
        .unwrap();
    assert!(!state.streams.contains_key(&1));
    assert_eq!(
        state.send(&owner, &stream, vec![1]),
        Err(SessionError::Protocol)
    );
}

// Invalid empty DATA fails only its active stream and does not produce a data event.
#[test]
fn empty_peer_data_is_a_stream_error() {
    let (mut state, registry, owner) = setup();
    let first = state.open(&registry, &owner).unwrap();
    let second = state.open(&registry, &owner).unwrap();
    state
        .receive_parts(FrameType::Data, first.wire_id().get(), vec![], &registry)
        .unwrap();
    assert!(!state.streams.contains_key(&first.wire_id().get()));
    state.send(&owner, &second, vec![1]).unwrap();
    assert!(state.events.iter().any(|e| matches!(
        e,
        Event::StreamClosed {
            outcome: StreamOutcome::Failed(Failure {
                code: FailureCode::StreamState,
                ..
            }),
            ..
        }
    )));
}

// Either peer can end first; the opposite direction works until both ENDs complete.
#[test]
fn independent_half_close_orders_and_write_completion() {
    for remote_first in [false, true] {
        let (mut state, registry, owner) = setup();
        let stream = state.open(&registry, &owner).unwrap();
        if remote_first {
            state
                .receive_parts(FrameType::End, 1, vec![], &registry)
                .unwrap();
            state.send(&owner, &stream, vec![1]).unwrap();
            state.end(&owner, &stream).unwrap();
        } else {
            state.end(&owner, &stream).unwrap();
            state
                .receive_parts(FrameType::Data, 1, vec![2], &registry)
                .unwrap();
            state
                .receive_parts(FrameType::End, 1, vec![], &registry)
                .unwrap();
        }
        assert!(state.streams.contains_key(&1));
        assert!(
            !state
                .events
                .iter()
                .any(|e| matches!(e, Event::StreamClosed { .. }))
        );
        assert_eq!(
            state.send(&owner, &stream, vec![1]),
            Err(SessionError::Protocol)
        );
        assert_eq!(state.end(&owner, &stream), Err(SessionError::Protocol));
        state.sent(FrameType::End, 1);
        state.sent(FrameType::End, 1);
        state.sent(FrameType::Data, 1);
        assert!(!state.streams.contains_key(&1));
        assert_eq!(
            state
                .events
                .iter()
                .filter(|e| matches!(
                    e,
                    Event::StreamClosed {
                        outcome: StreamOutcome::Completed,
                        ..
                    }
                ))
                .count(),
            1
        );
    }
}

// Repeated END, nonempty END and DATA after peer END fail only that active stream.
#[test]
fn invalid_peer_half_close_sequences_are_isolated() {
    for (kind, payload, end_first) in [
        (FrameType::End, vec![1], false),
        (FrameType::End, vec![], true),
        (FrameType::Data, vec![1], true),
    ] {
        let (mut state, registry, owner) = setup();
        state.open(&registry, &owner).unwrap();
        let unaffected = state.open(&registry, &owner).unwrap();
        if end_first {
            state
                .receive_parts(FrameType::End, 1, vec![], &registry)
                .unwrap();
        }
        state.receive_parts(kind, 1, payload, &registry).unwrap();
        assert!(!state.streams.contains_key(&1));
        state.send(&owner, &unaffected, vec![7]).unwrap();
        assert!(state.events.iter().any(|e| matches!(
            e,
            Event::StreamClosed {
                outcome: StreamOutcome::Failed(Failure {
                    code: FailureCode::StreamState,
                    ..
                }),
                ..
            }
        )));
    }
}
