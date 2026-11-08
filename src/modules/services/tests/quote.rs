//! Exact quote hashes and stream bindings survive validation without silently changing selected terms.
use super::{
    Error,
    assessment::assess,
    authority::{Action, Authority},
    authorization::authorize,
    contract::{Context, Limits, OperationKey},
    handler::{Blob, Peer},
    quote::{BoardCurrent, Issue, Quoter},
    test_support::selection::selected,
    value::Value,
};
use crate::modules::{board, identity::api::IdentityId};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
// Exact acceptance requires the original hash, stream, peer and still-current complete offering.
#[test]
fn exact_acceptance_and_stale_offering() {
    let (owner, selection, ba, registry) = selected(false);
    let local = IdentityId([2; 32]);
    let a = Arc::new(Authority::new());
    let grant = a.grant(local, Action::Invoke, 65536).unwrap();
    let c = Context {
        version: 1,
        grant: &grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let peer = Peer {
        local,
        remote: IdentityId([1; 32]),
        key: OperationKey {
            requester: selection.roles().0,
            connection: [8; 32],
            stream: 1,
        },
    };
    let make = |peer| {
        let auth = authorize(&c, &a, local, &registry, selection.clone(), peer, None).unwrap();
        assess(
            &c,
            &a,
            auth,
            &Value::String("input".into()),
            Limits::default(),
        )
        .unwrap()
    };
    let q = Quoter::new(
        local,
        a.clone(),
        Limits {
            quotes: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    let issue = |peer| Issue {
        assessed: make(peer),
        bytes: Blob::new(br#"{"type":"quote","maximumPrice":"7"}"#.to_vec(), 128).unwrap(),
        maximum: 7,
        expires: c.deadline,
    };
    let first = q.issue(&c, issue(peer)).unwrap();
    assert_eq!(first.bytes(), br#"{"type":"quote","maximumPrice":"7"}"#);
    assert!(matches!(q.issue(&c, issue(peer)), Err(Error::Conflict)));
    let other = Peer {
        key: OperationKey {
            stream: 2,
            ..peer.key
        },
        ..peer
    };
    assert!(matches!(q.issue(&c, issue(other)), Err(Error::Limit)));
    let bg = ba
        .grant(local, board::authority::Action::Select, 65536)
        .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: c.deadline,
        cancelled: false,
    };
    let current = BoardCurrent {
        owner: &owner,
        context: &bc,
    };
    assert!(matches!(
        q.accept(&c, &first, peer, [0; 32], &current),
        Err(Error::Denied)
    ));
    assert!(matches!(
        q.accept(&c, &first, other, first.hash(), &current),
        Err(Error::Denied)
    ));
    let accepted = q.accept(&c, &first, peer, first.hash(), &current).unwrap();
    assert_eq!(accepted.price(), super::pricing::Price::Fixed(7));
    assert_eq!(accepted.maximum(), 7);
    assert_eq!(accepted.bytes().bytes(), first.bytes());
    assert_eq!(accepted.assessed().authorized().key(), peer.key);
    assert!(matches!(
        q.accept(&c, &first, peer, first.hash(), &current),
        Err(Error::Conflict)
    ));
    let second = q.issue(&c, issue(other)).unwrap();
    let pg = ba
        .grant(local, board::authority::Action::Publish, 65536)
        .unwrap();
    let empty = board::prepared::Prepared::new(
        local,
        br#"{"version":1,"providedServices":[],"wantedServices":[]}"#,
        &BTreeMap::new(),
        Default::default(),
        registry.as_ref(),
    )
    .unwrap();
    owner
        .publish(&board::contract::Context { grant: &pg, ..bc }, empty)
        .unwrap();
    assert!(matches!(
        q.accept(&c, &second, other, second.hash(), &current),
        Err(Error::Conflict)
    ));
    q.disconnect(&c, peer.key.connection).unwrap();
    assert!(matches!(
        q.accept(&c, &second, other, second.hash(), &current),
        Err(Error::Conflict)
    ));
}
// Unaccepted expiry and an unrelated quote owner cannot grant acceptance on another stream domain.
#[test]
fn expired_and_foreign_quotes() {
    let (owner, s, ba, r) = selected(false);
    let local = IdentityId([2; 32]);
    let a = Arc::new(Authority::new());
    let g = a.grant(local, Action::Invoke, 65536).unwrap();
    let c = Context {
        version: 1,
        grant: &g,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let peer = Peer {
        local,
        remote: IdentityId([1; 32]),
        key: OperationKey {
            requester: s.roles().0,
            connection: [8; 32],
            stream: 1,
        },
    };
    let make = || {
        let auth = authorize(&c, &a, local, &r, s.clone(), peer, None).unwrap();
        assess(&c, &a, auth, &Value::String("x".into()), Limits::default()).unwrap()
    };
    let q = Quoter::new(local, a.clone(), Limits::default()).unwrap();
    assert!(matches!(
        q.issue(
            &c,
            Issue {
                assessed: make(),
                bytes: Blob::new(vec![1], 1).unwrap(),
                maximum: 7,
                expires: Instant::now()
            }
        ),
        Err(Error::Cancelled)
    ));
    let first = q
        .issue(
            &c,
            Issue {
                assessed: make(),
                bytes: Blob::new(vec![1], 1).unwrap(),
                maximum: 7,
                expires: c.deadline,
            },
        )
        .unwrap();
    let other = Quoter::new(local, a, Limits::default()).unwrap();
    let bg = ba
        .grant(local, board::authority::Action::Select, 65536)
        .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: c.deadline,
        cancelled: false,
    };
    let current = BoardCurrent {
        owner: &owner,
        context: &bc,
    };
    assert!(matches!(
        other.accept(&c, &first, peer, first.hash(), &current),
        Err(Error::Denied)
    ));
    let cancelled = Context {
        cancelled: true,
        ..c
    };
    assert!(matches!(
        q.accept(&cancelled, &first, peer, first.hash(), &current),
        Err(Error::Cancelled)
    ));
}
