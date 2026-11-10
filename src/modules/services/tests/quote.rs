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

struct TestClock(std::sync::Mutex<Instant>);
impl super::quote::Clock for TestClock {
    // The fixture controls local time explicitly without sleeping or using a peer clock.
    fn now(&self) -> Instant {
        *self.0.lock().unwrap()
    }
}
struct ExpireDuringCurrent(Arc<TestClock>, Instant);
impl super::quote::Current for ExpireDuringCurrent {
    // A potentially slow dependency callback advances monotonic time across quote expiry.
    fn check(&self, _: &board::selection::Selection) -> Result<(), Error> {
        *self.0.0.lock().unwrap() = self.1;
        Ok(())
    }
}
// Quote validity must still hold after a callback; a successful current-offering check cannot revive expiry.
#[test]
fn expiry_after_dependency_callback() {
    let (_, selection, _, registry) = selected(false);
    let local = IdentityId([2; 32]);
    let a = Arc::new(Authority::new());
    let g = a.grant(local, Action::Invoke, 65536).unwrap();
    let c = super::runtime_tests::context(&g);
    let peer = Peer {
        local,
        remote: IdentityId([1; 32]),
        key: OperationKey {
            requester: IdentityId([1; 32]),
            connection: [8; 32],
            stream: 1,
        },
    };
    let authorized = authorize(&c, &a, local, &registry, selection, peer, None).unwrap();
    let assessed = assess(
        &c,
        &a,
        authorized,
        &Value::String("input".into()),
        Limits::default(),
    )
    .unwrap();
    let now = Instant::now();
    let clock = Arc::new(TestClock(std::sync::Mutex::new(now)));
    let q = Quoter::with_clock(local, a, Limits::default(), clock.clone()).unwrap();
    let expires = now + Duration::from_secs(1);
    let quote = q
        .issue(
            &c,
            Issue {
                assessed,
                bytes: Blob::new(vec![1], 1).unwrap(),
                maximum: 7,
                expires,
            },
        )
        .unwrap();
    assert!(matches!(
        q.accept(
            &c,
            &quote,
            peer,
            quote.hash(),
            &ExpireDuringCurrent(clock, expires)
        ),
        Err(Error::Cancelled)
    ));
}

// Reusing identical public bytes/key cannot revive an older opaque generation or release its replacement slot.
#[test]
fn exact_quote_generation_and_discard() {
    let (owner, selection, ba, registry) = selected(false);
    let local = IdentityId([2; 32]);
    let authority = Arc::new(Authority::new());
    let g = authority.grant(local, Action::Invoke, 65536).unwrap();
    let c = super::runtime_tests::context(&g);
    let peer = Peer {
        local,
        remote: IdentityId([1; 32]),
        key: OperationKey {
            requester: IdentityId([1; 32]),
            connection: [8; 32],
            stream: 1,
        },
    };
    let q = Quoter::new(local, authority.clone(), Limits::default()).unwrap();
    let make = || {
        let auth = authorize(
            &c,
            &authority,
            local,
            &registry,
            selection.clone(),
            peer,
            None,
        )
        .unwrap();
        let assessed = assess(
            &c,
            &authority,
            auth,
            &Value::String("input".into()),
            Limits::default(),
        )
        .unwrap();
        q.issue(
            &c,
            Issue {
                assessed,
                bytes: Blob::new(vec![1], 1).unwrap(),
                maximum: 7,
                expires: c.deadline,
            },
        )
        .unwrap()
    };
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
    let first = make();
    assert_eq!(first.price(), super::pricing::Price::Fixed(7));
    q.accept(&c, &first, peer, first.hash(), &current).unwrap();
    let second = make();
    assert_eq!(first.hash(), second.hash());
    assert_eq!(q.discard(&c, &first, peer), Err(Error::Conflict));
    assert!(matches!(
        q.accept(&c, &first, peer, first.hash(), &current),
        Err(Error::Conflict)
    ));
    q.accept(&c, &second, peer, second.hash(), &current)
        .unwrap();
    let third = make();
    q.discard(&c, &third, peer).unwrap();
    assert!(matches!(
        q.accept(&c, &third, peer, third.hash(), &current),
        Err(Error::Conflict)
    ));
}
