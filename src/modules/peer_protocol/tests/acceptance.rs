//! Real two-sided reservations follow exact consent and survive send loss under the original operation key.
use super::{Error, base64, message::Message, test_support::invocation::Pair};
use crate::modules::services::{self, value::Value};
use std::sync::atomic::Ordering;
// Requester reservation precedes exposed accept bytes, provider reservation follows receipt, and neither executes early.
#[test]
fn exact_acceptance_and_reservations() {
    let pair = Pair::new(false);
    let (issued, request) = pair.propose(1, Value::String("original input".into()), 7);
    assert_eq!(pair.owned_available(), 100);
    assert_eq!(pair.provider.available(), 100);
    let association = issued.association();
    let hash = issued.hash();
    let outgoing = pair.reserve(&issued, request).unwrap();
    assert_eq!(pair.owned_available(), 93);
    assert_eq!(pair.provider.available(), 100);
    assert_eq!(outgoing.association().operation, association.operation);
    assert_eq!(outgoing.input(), &Value::String("original input".into()));
    assert_eq!(outgoing.invocation().quote.bytes(), issued.bytes());
    let accept = Message::parse(outgoing.accept(), 65536).unwrap();
    assert_eq!(accept.value()["quoteHash"], base64::encode(&hash));
    let incoming = pair.accept(issued, association, &accept).unwrap();
    assert_eq!(incoming.association(), association);
    assert_eq!(incoming.price(), services::pricing::Price::Fixed(7));
    assert_eq!(incoming.selection().offering().id, "fixture");
    assert!(incoming.input().descriptors().is_empty());
    assert_eq!(pair.provider.available(), 93);
    assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
    let p = &pair.provider;
    let observe = p
        .services
        .grant(p.local, services::authority::Action::Observe, 16_777_216)
        .unwrap();
    let key = super::acceptance::peer(association).key;
    assert_eq!(
        p.runtime
            .status(&super::test_support::world::service_context(&observe), key),
        Ok(Some(services::runtime::Phase::Reserved))
    );
    // Losing the send/session drops transient buffers but cannot erase the original requester's hold or dispatch marker.
    drop(outgoing);
    drop(incoming);
    assert_eq!(pair.owned_available(), 93);
    assert_eq!(pair.provider.available(), 93);
}
// Wrong IDs, exact-byte hashes and associations cannot reserve provider capacity; acceptance is not a reservation guarantee.
#[test]
fn invalid_or_unfunded_acceptance_has_no_provider_hold() {
    for case in 0..5 {
        let pair = Pair::new(false);
        let (issued, request) = pair.propose(1, Value::String("x".into()), 7);
        let mut observed = issued.association();
        let outgoing = pair.reserve(&issued, request).unwrap();
        let mut wire = Message::parse(outgoing.accept(), 65536)
            .unwrap()
            .value()
            .clone();
        match case {
            0 => wire["quoteId"] = serde_json::json!(base64::encode(&[0; 32])),
            1 => wire["quoteHash"] = serde_json::json!(base64::encode(&[0; 32])),
            2 => observed.operation.stream += 2,
            3 => observed.operation.connection = [0; 32],
            _ => {
                let p = &pair.provider;
                let admin = p
                    .ledger_authority
                    .grant(
                        p.local,
                        crate::modules::ledger::authority::Action::Administer,
                        16_777_216,
                    )
                    .unwrap();
                p.ledger
                    .revoke(
                        &super::test_support::world::ledger_context(&admin),
                        p.requester,
                        100,
                    )
                    .unwrap();
            }
        }
        let message = Message::parse(wire.to_string().as_bytes(), 65536).unwrap();
        let result = pair.accept(issued, observed, &message);
        assert!(
            matches!(result, Err(Error::Denied | Error::Insufficient)),
            "case {case}"
        );
        assert_eq!(pair.provider.available(), if case == 4 { 0 } else { 100 });
        assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
    }
}
// Two equal local negotiations on the same stream cannot grant a second requester dispatch or financial hold.
#[test]
fn duplicate_requester_admission_does_not_dispatch_twice() {
    let pair = Pair::new(false);
    let (issued, request) = pair.propose(1, Value::String("x".into()), 7);
    let outgoing = pair.reserve(&issued, request).unwrap();
    // Recreating a local proposal never changes the retained operation key into a fresh financial attempt.
    let (a, s, _) = pair.provider.proposal(1, &Value::String("x".into()));
    let r = &pair.requester;
    let pg = r
        .authority
        .grant(r.local, super::authority::Action::Request, 16_777_216)
        .unwrap();
    let sg = r
        .services
        .grant(r.local, services::authority::Action::Request, 16_777_216)
        .unwrap();
    let request = pair
        .client
        .request(
            &super::test_support::world::context(&pg),
            &super::test_support::world::service_context(&sg),
            super::contract::Association {
                local: r.local,
                peer: pair.provider.local,
                ..a
            },
            s,
            Value::String("x".into()),
            super::requester::Spending {
                maximum: 7,
                payment: Default::default(),
            },
        )
        .unwrap();
    assert!(matches!(
        pair.reserve(&issued, request),
        Err(Error::Conflict | Error::Unknown)
    ));
    assert_eq!(pair.owned_available(), 93);
    assert!(!outgoing.accept().is_empty());
}
