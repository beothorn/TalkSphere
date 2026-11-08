//! Literal LG acceptance supplements close boundaries beyond focused technical tests.
use super::*;
use crate::modules::identity::api::IdentityId;
use crate::modules::ledger::{
    accounting::Trust,
    authority::{Action, Authority, Grant},
    contract::{Context, Limits, OperationKey},
    fees::{Accepted, Peer, Quotes, Request},
    persistence::{Memory, Persistence},
    state::{Ledger, State},
    terms::{Policy, Terms},
    transfer::Order,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Each public call receives narrowly typed caller authority and a finite lifetime.
fn context(g: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant: g,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
struct Fixture {
    ledger: Ledger<Memory>,
    authority: Arc<Authority>,
    admin: Grant,
    read: Grant,
    coord: Grant,
    control: Grant,
}
impl Fixture {
    // Fresh complete fixture admits valid baseline work before mutating a single failure property.
    fn new(local: IdentityId, issuance: u64, internal: u64) -> Self {
        let authority = Arc::new(Authority::new());
        let admin = authority
            .grant(local, Action::Administer, 1_000_000)
            .unwrap();
        let read = authority.grant(local, Action::Read, 1_000_000).unwrap();
        let coord = authority
            .grant(local, Action::Coordinate, 1_000_000)
            .unwrap();
        let control = authority.grant(local, Action::Control, 1_000_000).unwrap();
        let ledger = Ledger::initialize(
            local,
            authority.clone(),
            Memory::default(),
            Limits::default(),
            &context(&admin),
            issuance,
            internal,
        )
        .unwrap();
        Self {
            ledger,
            authority,
            admin,
            read,
            coord,
            control,
        }
    }
}
// LG-02/LG-03 literal price7 settles each independent local role with finite budgets and no self-balances.
#[test]
fn literal_two_party_and_internal() {
    let [a, b, _, _] = test_support::identities();
    for (local, issuer, internal) in [
        (a, b, false),
        (b, b, false),
        (a, a, false),
        (b, a, false),
        (a, a, true),
    ] {
        let provider = if internal { a } else { b };
        let f = Fixture::new(local, if issuer == a { 10 } else { 100 }, 10);
        if local == b {
            f.ledger
                .set_trust(&context(&f.admin), a, Trust::new(1).unwrap())
                .unwrap();
            if issuer == b {
                f.ledger.grant(&context(&f.admin), a, 100).unwrap();
            }
        } else if issuer == b {
            f.ledger.set_owned(&context(&f.admin), b, 100).unwrap();
        }
        let key = OperationKey {
            requester: a,
            connection: [1; 32],
            stream: 1,
        };
        let r = f
            .ledger
            .reserve(
                &context(&f.coord),
                key,
                Terms {
                    requester: a,
                    provider,
                    issuer,
                    maximum: 7,
                    fixed: true,
                    accepted: vec![7],
                    transfer: None,
                    fee: None,
                    spending_bound: 7,
                },
                Policy {
                    requester_credits: true,
                    third_party: false,
                },
            )
            .unwrap();
        if issuer == b {
            assert_eq!(
                f.ledger.available(
                    &context(&f.read),
                    if local == a { b } else { a },
                    local == a
                ),
                Ok(93)
            );
        }
        let token = f
            .ledger
            .prepare(&context(&f.coord), &r, 7, vec![1])
            .unwrap();
        f.ledger.commit(&context(&f.coord), &token).unwrap();
        f.ledger.commit(&context(&f.coord), &token).unwrap();
        if internal {
            assert_eq!(f.ledger.budgets(&context(&f.read)), Ok((10, 3)));
        } else if issuer == b {
            let view = f
                .ledger
                .view(&context(&f.read), if local == a { b } else { a })
                .unwrap();
            assert_eq!(if local == a { view.owned } else { view.issued }, 93);
        } else if local == a {
            assert_eq!(f.ledger.view(&context(&f.read), b).unwrap().issued, 7);
            assert_eq!(f.ledger.capacities(&context(&f.read)).unwrap().0, 3);
        } else {
            assert_eq!(f.ledger.view(&context(&f.read), a).unwrap().owned, 7);
        }
        let state = &f.ledger.inner.lock().unwrap().state;
        assert!(!state.accounts.owned.contains_key(&local));
        assert!(!state.accounts.issued.contains_key(&local));
    }
}
// LG-05 exact maximum spending and every invalid administrative call preserve exact accounting.
#[test]
fn exact_maximum_and_authority() {
    let [a, b, _, _] = test_support::identities();
    let f = Fixture::new(a, 10, 10);
    for input in ["-1", "18446744073709551616", "1.5"] {
        let v: serde_json::Value = serde_json::from_str(input).unwrap();
        assert!(state::number(&v).is_err());
    }
    f.ledger.set_owned(&context(&f.admin), b, u64::MAX).unwrap();
    let mut terms = Terms {
        requester: a,
        provider: b,
        issuer: b,
        maximum: u64::MAX,
        fixed: true,
        accepted: vec![1],
        transfer: None,
        fee: None,
        spending_bound: u64::MAX,
    };
    let key = OperationKey {
        requester: a,
        connection: [1; 32],
        stream: 1,
    };
    let r = f
        .ledger
        .reserve(&context(&f.coord), key, terms.clone(), Policy::default())
        .unwrap();
    assert_eq!(f.ledger.available(&context(&f.read), b, true), Ok(0));
    assert!(
        f.ledger
            .prepare(&context(&f.coord), &r, 0, vec![1])
            .is_err()
    );
    let p = f
        .ledger
        .prepare(&context(&f.coord), &r, u64::MAX, vec![1])
        .unwrap();
    f.ledger.commit(&context(&f.coord), &p).unwrap();
    assert_eq!(f.ledger.view(&context(&f.read), b).unwrap().owned, 0);
    terms.maximum = 1;
    terms.spending_bound = 1;
    assert!(
        f.ledger
            .reserve(
                &context(&f.coord),
                OperationKey { stream: 2, ..key },
                terms,
                Policy::default()
            )
            .is_err()
    );
    assert_eq!(
        f.ledger.grant(&context(&f.control), b, 1),
        Err(Error::Denied)
    );
    assert_eq!(
        f.ledger.revoke(&context(&f.control), b, 1),
        Err(Error::Denied)
    );
    assert_eq!(
        f.ledger
            .set_trust(&context(&f.control), b, Trust::new(1).unwrap()),
        Err(Error::Denied)
    );
}
// LG-06 authentication/balance alone grants no trust; bounded control and explicit finite grant5 remain separate.
#[test]
fn explicit_bootstrap() {
    let [a, b, _, _] = test_support::identities();
    let f = Fixture::new(b, 200, 10);
    f.ledger.grant(&context(&f.admin), a, 100).unwrap();
    let key = OperationKey {
        requester: a,
        connection: [1; 32],
        stream: 1,
    };
    let t = Terms {
        requester: a,
        provider: b,
        issuer: b,
        maximum: 7,
        fixed: true,
        accepted: vec![1],
        transfer: None,
        fee: None,
        spending_bound: 7,
    };
    assert_eq!(
        f.ledger
            .reserve(&context(&f.coord), key, t.clone(), Policy::default()),
        Err(Error::Denied)
    );
    assert_eq!(f.ledger.balance(&context(&f.control), a), Ok(100));
    f.ledger.grant(&context(&f.admin), a, 5).unwrap();
    assert_eq!(f.ledger.balance(&context(&f.control), a), Ok(105));
    assert_eq!(
        f.ledger
            .view(&context(&f.read), a)
            .unwrap()
            .trust
            .millionths(),
        0
    );
    f.ledger
        .set_trust(&context(&f.admin), a, Trust::new(1).unwrap())
        .unwrap();
    f.ledger
        .reserve(&context(&f.coord), key, t, Policy::default())
        .unwrap();
}
// LG-07/LG-09 compose real signatures, exact fee acceptance, committed replay recognition and maintenance/restart.
#[test]
fn signed_transfer_journey() {
    use crate::modules::cryptography::{
        credential,
        primitives::{keys::signing_key, signature},
        test_support::ScriptedEntropy,
    };
    let mut entropy = ScriptedEntropy(std::collections::VecDeque::from([Ok(vec![1; 32])]));
    let key = signing_key(&mut entropy).unwrap();
    let owner = IdentityId(credential::identity_id(&key.public_key).unwrap());
    let [_, new_owner, issuer, _] = test_support::identities();
    let f = Fixture::new(issuer, 100, 10);
    f.ledger.grant(&context(&f.admin), owner, 100).unwrap();
    let order = Order {
        owner,
        new_owner,
        issuer,
        transfer_id: [9; 16],
        amount: 9,
    };
    let signature = signature::sign(&key.private_key, &order.encode().unwrap(), 121).unwrap();
    let peer = Peer {
        owner,
        connection: [1; 32],
        stream: 1,
    };
    assert_eq!(
        f.ledger.existing_transfer(
            &context(&f.control),
            owner,
            &order,
            &key.public_key,
            &signature
        ),
        Ok(None)
    );
    let gate = Quotes::new(issuer, f.authority.clone(), 8).unwrap();
    let quote = gate
        .quote(
            &context(&f.control),
            peer,
            Request {
                order: order.clone(),
                key: key.public_key,
                signature,
                maximum_fee: 2,
                fee: 2,
                expires: 20,
            },
            10,
        )
        .unwrap();
    assert_eq!(f.ledger.balance(&context(&f.control), owner), Ok(100));
    let accepted = gate
        .accept(&context(&f.control), &quote, peer, 2, 10)
        .unwrap();
    let result = f
        .ledger
        .commit_transfer(&context(&f.control), &accepted)
        .unwrap();
    assert_eq!(result.received(), Ok(7));
    assert_eq!(f.ledger.balance(&context(&f.control), owner), Ok(91));
    assert_eq!(f.ledger.balance(&context(&f.control), new_owner), Ok(7));
    f.ledger.maintain(&context(&f.admin)).unwrap();
    let image = f.ledger.inner.lock().unwrap().persistence.load().unwrap();
    let reopened = Ledger::open(
        issuer,
        f.authority.clone(),
        Memory(image),
        Limits::default(),
        &context(&f.read),
    )
    .unwrap();
    assert_eq!(
        reopened.existing_transfer(
            &context(&f.control),
            owner,
            &order,
            &key.public_key,
            &signature
        ),
        Ok(Some(result))
    );
    let zero_order = Order {
        transfer_id: [10; 16],
        ..order.clone()
    };
    let zero_signature =
        signature::sign(&key.private_key, &zero_order.encode().unwrap(), 121).unwrap();
    let zero_peer = Peer { stream: 2, ..peer };
    let zero_quote = gate
        .quote(
            &context(&f.control),
            zero_peer,
            Request {
                order: zero_order,
                key: key.public_key,
                signature: zero_signature,
                maximum_fee: 0,
                fee: 0,
                expires: 20,
            },
            11,
        )
        .unwrap();
    let zero_accept = gate
        .accept(&context(&f.control), &zero_quote, zero_peer, 0, 11)
        .unwrap();
    assert_eq!(
        reopened
            .commit_transfer(&context(&f.control), &zero_accept)
            .unwrap()
            .received(),
        Ok(9)
    );
    assert_eq!(reopened.balance(&context(&f.control), owner), Ok(82));
    assert_eq!(reopened.balance(&context(&f.control), new_owner), Ok(16));
    let changed = Order { amount: 8, ..order };
    let signature = signature::sign(&key.private_key, &changed.encode().unwrap(), 121).unwrap();
    assert_eq!(
        reopened.existing_transfer(
            &context(&f.control),
            owner,
            &changed,
            &key.public_key,
            &signature
        ),
        Err(Error::Conflict)
    );
}
// LG-08 independent accounting refusal cases start from affordable valid sources without capacity masking.
#[test]
fn transfer_accounting_refusals() {
    let [a, b, c, _] = test_support::identities();
    for case in 0..4 {
        let f = Fixture::new(c, 100, 10);
        f.ledger.grant(&context(&f.admin), a, 100).unwrap();
        if case == 3 {
            let mut initial = State::default();
            initial.accounts.issued.insert(a, 100);
            initial.accounts.issued.insert(b, u64::MAX);
            let record = initial.record(c, Limits::default()).unwrap();
            let mut inner = f.ledger.inner.lock().unwrap();
            inner.persistence.commit(&record).unwrap();
            inner.state = initial;
        }
        let mut order = Order {
            owner: a,
            new_owner: b,
            issuer: c,
            transfer_id: [9; 16],
            amount: 9,
        };
        let fee = if case == 1 { 9 } else { 2 };
        if case == 0 {
            order.amount = 101
        }
        if case == 2 {
            order.new_owner = a
        }
        let before = f.ledger.inner.lock().unwrap().persistence.load().unwrap();
        let result = f.ledger.commit_transfer(
            &context(&f.control),
            &Accepted {
                authority: f.authority.clone(),
                order,
                fee,
            },
        );
        assert!(result.is_err());
        assert_eq!(
            f.ledger.inner.lock().unwrap().persistence.load().unwrap(),
            before
        );
    }
}

// Exact incoming headroom permits the uint64 boundary and rejects the next credit before work.
#[test]
fn incoming_exact_maximum() {
    let [a, b, _, _] = test_support::identities();
    let f = Fixture::new(b, 10, 10);
    f.ledger
        .set_trust(&context(&f.admin), a, Trust::new(1).unwrap())
        .unwrap();
    f.ledger
        .set_owned(&context(&f.admin), a, u64::MAX - 7)
        .unwrap();
    let key = OperationKey {
        requester: a,
        connection: [1; 32],
        stream: 1,
    };
    let terms = Terms {
        requester: a,
        provider: b,
        issuer: a,
        maximum: 7,
        fixed: true,
        accepted: vec![1],
        transfer: None,
        fee: None,
        spending_bound: 7,
    };
    let policy = Policy {
        requester_credits: true,
        third_party: false,
    };
    let r = f
        .ledger
        .reserve(&context(&f.coord), key, terms.clone(), policy)
        .unwrap();
    let mut forged = r.clone();
    forged.local = a;
    assert_eq!(
        f.ledger.prepare(&context(&f.coord), &forged, 7, vec![1]),
        Err(Error::Denied)
    );
    let p = f
        .ledger
        .prepare(&context(&f.coord), &r, 7, vec![1])
        .unwrap();
    f.ledger.commit(&context(&f.coord), &p).unwrap();
    assert_eq!(f.ledger.view(&context(&f.read), a).unwrap().owned, u64::MAX);
    assert!(
        f.ledger
            .reserve(
                &context(&f.coord),
                OperationKey { stream: 2, ..key },
                Terms {
                    maximum: 1,
                    spending_bound: 1,
                    ..terms
                },
                policy
            )
            .is_err()
    );
}
