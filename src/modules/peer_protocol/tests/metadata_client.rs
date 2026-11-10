//! Direct publisher attribution, exact raw documents and receipt races through actual Board cache contracts.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Association, Context, OperationKey},
    message::Message,
    metadata,
    metadata_client::Request,
    record::Record,
    test_support::board::{context as board_context, published},
};
use crate::modules::{
    board::{
        self,
        cache::{Cache, Provenance},
    },
    identity::api::IdentityId,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Contexts come from separate locally issued requester and incoming-event grants, never from JSON.
fn context(grant: &super::authority::Grant) -> Context<'_> {
    Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// Compose exact real Board responses without attributing arbitrary document fields to an authenticated publisher.
fn reply(owner: &board::state::Provider, c: &board::contract::Context<'_>, raw: &[u8]) -> Message {
    let request = Message::parse(raw, 65536).unwrap();
    let Record::Control(bytes) = metadata::respond(owner, c, &request).unwrap() else {
        panic!("control reply");
    };
    Message::parse(&bytes, 65536).unwrap()
}
// Wrong stream/connection/identity and outgoing-only grants cannot install a direct cache receipt or mix definition revisions.
#[test]
fn publisher_attribution_and_stale_definitions() {
    let (owner, ba, peer) = published();
    let local = IdentityId([1; 32]);
    let read = ba
        .grant(peer, board::authority::Action::Read, 1_000_000)
        .unwrap();
    let bc = board_context(&read);
    let authority = Arc::new(Authority::new());
    let request = authority.grant(local, Action::Request, 1_000_000).unwrap();
    let incoming = authority.grant(local, Action::Invoke, 1_000_000).unwrap();
    let association = Association {
        local,
        peer,
        operation: OperationKey {
            requester: local,
            connection: [7; 32],
            stream: 1,
        },
    };
    let ca = Arc::new(board::authority::Authority::new());
    let cache_grant = ca
        .grant(peer, board::authority::Action::Cache, 1_000_000)
        .unwrap();
    let cc = board_context(&cache_grant);
    let mut cache = Cache::new(ca.clone(), Default::default()).unwrap();
    let response = reply(&owner, &bc, br#"{"type":"board.get"}"#);
    let query = Request::board(&context(&request), authority.clone(), association).unwrap();
    assert_eq!(query.wire(), serde_json::json!({"type":"board.get"}));
    assert!(matches!(
        query.apply(
            &context(&request),
            association,
            &response,
            &mut cache,
            &cc,
            10
        ),
        Err(Error::Denied)
    ));
    let query = Request::board(&context(&request), authority.clone(), association).unwrap();
    let receipt = query
        .apply(
            &context(&incoming),
            association,
            &response,
            &mut cache,
            &cc,
            10,
        )
        .unwrap();
    assert_eq!(receipt.provenance(), Provenance::Direct(peer));
    assert_eq!(receipt.bytes(), owner.read(&bc).unwrap().bytes());
    for wrong in [
        Association {
            peer: IdentityId([3; 32]),
            ..association
        },
        Association {
            operation: OperationKey {
                stream: 2,
                ..association.operation
            },
            ..association
        },
        Association {
            operation: OperationKey {
                connection: [8; 32],
                ..association.operation
            },
            ..association
        },
    ] {
        let query = Request::board(&context(&request), authority.clone(), association).unwrap();
        assert!(matches!(
            query.apply(&context(&incoming), wrong, &response, &mut cache, &cc, 11),
            Err(Error::Denied)
        ));
    }
    let definition = reply(
        &owner,
        &bc,
        br#"{"type":"service.get","offeringId":"fixture"}"#,
    );
    let next = Association {
        operation: OperationKey {
            stream: 2,
            ..association.operation
        },
        ..association
    };
    let query = Request::definition(
        &context(&request),
        authority.clone(),
        next,
        "fixture",
        receipt.clone(),
    )
    .unwrap();
    assert_eq!(
        query.wire(),
        serde_json::json!({"type":"service.get","offeringId":"fixture"})
    );
    let with_definition = query
        .apply(&context(&incoming), next, &definition, &mut cache, &cc, 12)
        .unwrap();
    assert_eq!(with_definition.definition_count(), 1);
    let forwarded = cache
        .store(&cc, Provenance::Forwarded(peer), receipt.bytes(), 12)
        .unwrap();
    assert!(matches!(
        Request::definition(
            &context(&request),
            authority.clone(),
            next,
            "fixture",
            forwarded
        ),
        Err(Error::Denied)
    ));
    let pending = Request::definition(
        &context(&request),
        authority.clone(),
        next,
        "fixture",
        with_definition,
    )
    .unwrap();
    cache
        .store(&cc, Provenance::Direct(peer), receipt.bytes(), 13)
        .unwrap();
    assert!(matches!(
        pending.apply(&context(&incoming), next, &definition, &mut cache, &cc, 14),
        Err(Error::Conflict)
    ));
    let pending = Request::board(&context(&request), authority.clone(), association).unwrap();
    authority.revoke();
    assert!(matches!(
        pending.apply(
            &context(&incoming),
            association,
            &response,
            &mut cache,
            &cc,
            14
        ),
        Err(Error::Denied)
    ));
}
// A raw subdocument span preserves whitespace and full integers; no serialization round trip is used for cache input.
#[test]
fn exact_subdocument_source() {
    let raw=br#"{ "type":"board", "board": { "version":1, "providedServices":[], "wantedServices":[], "unknown":18446744073709551615 } }"#;
    let response = Message::parse(raw, 65536).unwrap();
    assert_eq!(response.field_bytes("board").unwrap().unwrap(),br#" { "version":1, "providedServices":[], "wantedServices":[], "unknown":18446744073709551615 } "#);
    assert_eq!(response.field_bytes("absent"), Ok(None));
    assert_eq!(
        response.value()["board"]["unknown"].to_string(),
        "18446744073709551615"
    );
}
