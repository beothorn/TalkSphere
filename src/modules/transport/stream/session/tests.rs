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
        state.receive_parts(FrameType::Data, 1, vec![1], &registry),
        Err(SessionError::Protocol)
    );
}
