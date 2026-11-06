//! Observable provider reservation behavior with real account authority and durable images.
use super::super::{
    accounting::Trust,
    authority::{Authority, Grant},
    contract::Limits,
    persistence::Memory,
};
use super::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Explicit caller scope for one bounded operation.
fn context(g: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant: g,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// A valid accepted price uses independent literal quote bytes and finite local capacities.
fn terms(a: IdentityId, b: IdentityId) -> Terms {
    Terms {
        requester: a,
        provider: b,
        issuer: b,
        maximum: 60,
        fixed: false,
        accepted: b"literal accepted quote".to_vec(),
        transfer: None,
        fee: None,
        spending_bound: 100,
    }
}
// Both local views reserve without debit; exact retries reuse one hold and restart retains it.
#[test]
fn provider_holds_and_replay() {
    let [a, b, _, _] = super::super::test_support::identities();
    for local in [a, b] {
        let authority = Arc::new(Authority::new());
        let admin = authority.grant(local, Action::Administer, 65536).unwrap();
        let read = authority.grant(local, Action::Read, 65536).unwrap();
        let coord = authority.grant(local, Action::Coordinate, 65536).unwrap();
        let limits = Limits {
            accounts: 16,
            operations: 16,
            transfers: 16,
            claims: 16,
            quotes: 8,
            state_bytes: 65536,
        };
        let ledger = Ledger::initialize(
            local,
            authority.clone(),
            Memory::default(),
            limits,
            &context(&admin),
            100,
            100,
        )
        .unwrap();
        if local == a {
            ledger.set_owned(&context(&admin), b, 100).unwrap()
        } else {
            ledger.grant(&context(&admin), a, 100).unwrap();
            ledger
                .set_trust(&context(&admin), a, Trust::new(1).unwrap())
                .unwrap();
        }
        let k = OperationKey {
            requester: a,
            connection: [1; 32],
            stream: 1,
        };
        let t = terms(a, b);
        let r = ledger
            .reserve(&context(&coord), k, t.clone(), Policy::default())
            .unwrap();
        assert_eq!(
            ledger.reserve(&context(&coord), k, t.clone(), Policy::default()),
            Ok(r)
        );
        let peer = if local == a { b } else { a };
        assert_eq!(ledger.available(&context(&read), peer, local == a), Ok(40));
        let mut changed = t.clone();
        changed.maximum = 61;
        assert_eq!(
            ledger.reserve(&context(&coord), k, changed, Policy::default()),
            Err(Error::Conflict)
        );
        assert_eq!(
            ledger.reserve(
                &context(&coord),
                OperationKey { stream: 2, ..k },
                t,
                Policy::default()
            ),
            Err(Error::Insufficient)
        );
        if local == a {
            assert_eq!(
                ledger.set_owned(&context(&admin), b, 59),
                Err(Error::Insufficient)
            );
        } else {
            assert_eq!(
                ledger.revoke(&context(&admin), a, 41),
                Err(Error::Insufficient)
            );
        }
        let image = ledger.inner.lock().unwrap().persistence.load().unwrap();
        let reopened = Ledger::open(
            local,
            authority.clone(),
            Memory(image),
            limits,
            &context(&read),
        )
        .unwrap();
        assert_eq!(
            reopened.available(&context(&read), peer, local == a),
            Ok(40)
        );
    }
}
// Simultaneous individually affordable reservations cannot collectively exceed one account.
#[test]
fn concurrent_spending() {
    let [a, b, _, _] = super::super::test_support::identities();
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let coord = Arc::new(auth.grant(a, Action::Coordinate, 65536).unwrap());
    let ledger = Arc::new(
        Ledger::initialize(
            a,
            auth.clone(),
            Memory::default(),
            Limits {
                accounts: 16,
                operations: 16,
                transfers: 16,
                claims: 16,
                quotes: 8,
                state_bytes: 65536,
            },
            &context(&admin),
            100,
            100,
        )
        .unwrap(),
    );
    ledger.set_owned(&context(&admin), b, 100).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let mut threads = vec![];
    for stream in [1, 2] {
        let (l, g, barrier) = (ledger.clone(), coord.clone(), barrier.clone());
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            l.reserve(
                &context(&g),
                OperationKey {
                    requester: a,
                    connection: [1; 32],
                    stream,
                },
                terms(a, b),
                Policy::default(),
            )
        }));
    }
    assert_eq!(
        threads
            .into_iter()
            .filter(|t| t.thread().id() != std::thread::current().id())
            .map(|t| t.join().unwrap())
            .filter(Result::is_ok)
            .count(),
        1
    );
}

// Requester issuance is finite and provider incoming headroom stays reserved against reconciliation.
#[test]
fn requester_capacity() {
    let [a, b, _, _] = super::super::test_support::identities();
    for local in [a, b] {
        let auth = Arc::new(Authority::new());
        let admin = auth.grant(local, Action::Administer, 65536).unwrap();
        let coord = auth.grant(local, Action::Coordinate, 65536).unwrap();
        let ledger = Ledger::initialize(
            local,
            auth.clone(),
            Memory::default(),
            Limits {
                accounts: 16,
                operations: 16,
                transfers: 16,
                claims: 16,
                quotes: 8,
                state_bytes: 65536,
            },
            &context(&admin),
            10,
            10,
        )
        .unwrap();
        if local == b {
            ledger
                .set_trust(&context(&admin), a, Trust::new(1).unwrap())
                .unwrap();
            ledger.set_owned(&context(&admin), a, u64::MAX - 7).unwrap();
        }
        let mut t = terms(a, b);
        t.issuer = a;
        t.maximum = 7;
        let key = OperationKey {
            requester: a,
            connection: [1; 32],
            stream: 1,
        };
        let policy = Policy {
            requester_credits: true,
            third_party: false,
        };
        ledger
            .reserve(&context(&coord), key, t.clone(), policy)
            .unwrap();
        assert!(
            ledger
                .reserve(
                    &context(&coord),
                    OperationKey { stream: 2, ..key },
                    t,
                    policy
                )
                .is_err()
        );
        if local == a {
            assert_eq!(
                ledger.grant(&context(&admin), b, 4),
                Err(Error::Insufficient)
            );
        } else {
            assert_eq!(
                ledger.set_owned(&context(&admin), a, u64::MAX - 6),
                Err(Error::Limit)
            );
        }
    }
}

// Same-identity work consumes finite internal capacity without producing a currency self-balance.
#[test]
fn internal_budget() {
    let [a, _, _, _] = super::super::test_support::identities();
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let read = auth.grant(a, Action::Read, 65536).unwrap();
    let coord = auth.grant(a, Action::Coordinate, 65536).unwrap();
    let ledger = Ledger::initialize(
        a,
        auth.clone(),
        Memory::default(),
        Limits {
            accounts: 16,
            operations: 16,
            transfers: 16,
            claims: 16,
            quotes: 8,
            state_bytes: 65536,
        },
        &context(&admin),
        10,
        10,
    )
    .unwrap();
    let mut t = terms(a, a);
    t.maximum = 7;
    let key = OperationKey {
        requester: a,
        connection: [1; 32],
        stream: 1,
    };
    ledger
        .reserve(&context(&coord), key, t.clone(), Policy::default())
        .unwrap();
    assert!(
        ledger
            .reserve(
                &context(&coord),
                OperationKey { stream: 2, ..key },
                t,
                Policy::default()
            )
            .is_err()
    );
    assert_eq!(ledger.budgets(&context(&read)), Ok((10, 10)));
    assert!(ledger.inner.lock().unwrap().state.accounts.owned.is_empty());
    assert!(
        ledger
            .inner
            .lock()
            .unwrap()
            .state
            .accounts
            .issued
            .is_empty()
    );
}

// Third-party payment binds one transfer identity once and reserves the exact fee-inclusive gross.
#[test]
fn third_party_binding() {
    let [a, b, c, _] = super::super::test_support::identities();
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let read = auth.grant(a, Action::Read, 65536).unwrap();
    let coord = auth.grant(a, Action::Coordinate, 65536).unwrap();
    let ledger = Ledger::initialize(
        a,
        auth.clone(),
        Memory::default(),
        Limits {
            accounts: 16,
            operations: 16,
            transfers: 16,
            claims: 16,
            quotes: 8,
            state_bytes: 65536,
        },
        &context(&admin),
        100,
        100,
    )
    .unwrap();
    ledger.set_owned(&context(&admin), c, 100).unwrap();
    let mut t = terms(a, b);
    t.issuer = c;
    t.maximum = 7;
    t.fee = Some(2);
    t.transfer = Some([9; 16]);
    let key = OperationKey {
        requester: a,
        connection: [1; 32],
        stream: 1,
    };
    ledger
        .reserve(
            &context(&coord),
            key,
            t.clone(),
            Policy {
                requester_credits: false,
                third_party: true,
            },
        )
        .unwrap();
    assert_eq!(ledger.available(&context(&read), c, true), Ok(91));
    assert_eq!(
        ledger.reserve(
            &context(&coord),
            OperationKey { stream: 2, ..key },
            t,
            Policy {
                requester_credits: false,
                third_party: true
            }
        ),
        Err(Error::Conflict)
    );
}
