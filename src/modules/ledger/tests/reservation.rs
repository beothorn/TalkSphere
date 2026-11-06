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

// Exact preparations survive restart and reject conflicting final prices or references.
#[test]
fn prepared_status_and_conflicts() {
    let [a, b, _, _] = super::super::test_support::identities();
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let read = auth.grant(a, Action::Read, 65536).unwrap();
    let coord = auth.grant(a, Action::Coordinate, 65536).unwrap();
    let limits = Limits {
        accounts: 16,
        operations: 16,
        transfers: 16,
        claims: 16,
        quotes: 8,
        state_bytes: 65536,
    };
    let ledger = Ledger::initialize(
        a,
        auth.clone(),
        Memory::default(),
        limits,
        &context(&admin),
        100,
        100,
    )
    .unwrap();
    ledger.set_owned(&context(&admin), b, 100).unwrap();
    let key = OperationKey {
        requester: a,
        connection: [1; 32],
        stream: 1,
    };
    assert_eq!(ledger.status(&context(&coord), key), Ok(None));
    let r = ledger
        .reserve(&context(&coord), key, terms(a, b), Policy::default())
        .unwrap();
    assert_eq!(
        ledger.status(&context(&coord), key),
        Ok(Some(Status::Reserved))
    );
    assert!(ledger.prepare(&context(&coord), &r, 61, vec![1]).is_err());
    let token = ledger
        .prepare(&context(&coord), &r, 21, vec![1, 2, 3])
        .unwrap();
    assert_eq!(
        ledger.prepare(&context(&coord), &r, 21, vec![1, 2, 3]),
        Ok(token)
    );
    assert_eq!(
        ledger.prepare(&context(&coord), &r, 22, vec![1, 2, 3]),
        Err(Error::Conflict)
    );
    assert_eq!(
        ledger.prepare(&context(&coord), &r, 21, vec![2]),
        Err(Error::Conflict)
    );
    assert_eq!(ledger.available(&context(&read), b, true), Ok(40));
    let image = ledger.inner.lock().unwrap().persistence.load().unwrap();
    let reopened = Ledger::open(a, auth.clone(), Memory(image), limits, &context(&read)).unwrap();
    assert_eq!(
        reopened.status(&context(&coord), key),
        Ok(Some(Status::Prepared))
    );
}

// Every two-party/internal role applies its price once and releases unused metered capacity.
#[test]
fn local_commit_modes() {
    let [a, b, _, _] = super::super::test_support::identities();
    for (local, issuer, internal) in [
        (a, b, false),
        (b, b, false),
        (a, a, false),
        (b, a, false),
        (a, a, true),
    ] {
        let provider = if internal { a } else { b };
        let auth = Arc::new(Authority::new());
        let admin = auth.grant(local, Action::Administer, 65536).unwrap();
        let read = auth.grant(local, Action::Read, 65536).unwrap();
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
            100,
            100,
        )
        .unwrap();
        if !internal && local == b {
            ledger
                .set_trust(&context(&admin), a, Trust::new(1).unwrap())
                .unwrap();
            if issuer == b {
                ledger.grant(&context(&admin), a, 100).unwrap();
            }
        }
        if !internal && local == a && issuer == b {
            ledger.set_owned(&context(&admin), b, 100).unwrap();
        }
        let mut t = terms(a, provider);
        t.issuer = issuer;
        let key = OperationKey {
            requester: a,
            connection: [1; 32],
            stream: 1,
        };
        let r = ledger
            .reserve(
                &context(&coord),
                key,
                t,
                Policy {
                    requester_credits: true,
                    third_party: true,
                },
            )
            .unwrap();
        let token = ledger.prepare(&context(&coord), &r, 21, vec![1]).unwrap();
        ledger.commit(&context(&coord), &token).unwrap();
        ledger.commit(&context(&coord), &token).unwrap();
        assert_eq!(
            ledger.status(&context(&coord), key),
            Ok(Some(Status::Committed))
        );
        if internal {
            assert_eq!(ledger.budgets(&context(&read)), Ok((100, 79)));
        } else if issuer == b {
            let peer = if local == a { b } else { a };
            assert_eq!(ledger.available(&context(&read), peer, local == a), Ok(79));
        } else if local == a {
            assert_eq!(ledger.view(&context(&read), b).unwrap().issued, 21);
            assert_eq!(ledger.budgets(&context(&read)).unwrap().0, 79);
        } else {
            assert_eq!(ledger.view(&context(&read), a).unwrap().owned, 21);
        }
    }
}

// Explicit abort releases only uncommitted capacity; prepared timeout and committed cancel cannot erase effects.
#[test]
fn abort_decisions() {
    let [a, b, _, _] = super::super::test_support::identities();
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
    ledger.set_owned(&context(&admin), b, 100).unwrap();
    for stream in 1..=3 {
        let key = OperationKey {
            requester: a,
            connection: [1; 32],
            stream,
        };
        let r = ledger
            .reserve(&context(&coord), key, terms(a, b), Policy::default())
            .unwrap();
        if stream == 1 {
            ledger.abort(&context(&coord), &r, None).unwrap();
            ledger.abort(&context(&coord), &r, None).unwrap();
            assert_eq!(ledger.available(&context(&read), b, true), Ok(100));
        } else {
            let token = ledger.prepare(&context(&coord), &r, 21, vec![1]).unwrap();
            assert_eq!(ledger.abort(&context(&coord), &r, None), Err(Error::Denied));
            let mut expired = context(&coord);
            expired.cancelled = true;
            assert_eq!(
                ledger.abort(&expired, &r, Some(&token)),
                Err(Error::Cancelled)
            );
            assert_eq!(ledger.available(&context(&read), b, true), Ok(40));
            if stream == 2 {
                ledger.abort(&context(&coord), &r, Some(&token)).unwrap();
                assert_eq!(
                    ledger.commit(&context(&coord), &token),
                    Err(Error::Conflict)
                );
            } else {
                ledger.commit(&context(&coord), &token).unwrap();
                assert_eq!(
                    ledger.abort(&context(&coord), &r, Some(&token)),
                    Err(Error::Conflict)
                );
            }
        }
    }
}

// A test coordinator retains its decision across every SV-09 stage and retries delivery, never execution.
#[test]
fn coordinator_fault_matrix() {
    let [a, b, _, _] = super::super::test_support::identities();
    for cut in 0..8 {
        let auth = Arc::new(Authority::new());
        let admin = auth.grant(a, Action::Administer, 65536).unwrap();
        let read = auth.grant(a, Action::Read, 65536).unwrap();
        let coord = auth.grant(a, Action::Coordinate, 65536).unwrap();
        let limits = Limits {
            accounts: 16,
            operations: 16,
            transfers: 16,
            claims: 16,
            quotes: 8,
            state_bytes: 65536,
        };
        let ledger = Ledger::initialize(
            a,
            auth.clone(),
            Memory::default(),
            limits,
            &context(&admin),
            100,
            100,
        )
        .unwrap();
        ledger.set_owned(&context(&admin), b, 100).unwrap();
        let key = OperationKey {
            requester: a,
            connection: [1; 32],
            stream: 1,
        };
        let mut handler_prepared = false;
        let mut handler_visible = false;
        let commit_decision = cut >= 4;
        let r = ledger
            .reserve(&context(&coord), key, terms(a, b), Policy::default())
            .unwrap();
        if cut >= 2 {
            handler_prepared = true;
        }
        let token = if cut >= 3 {
            Some(ledger.prepare(&context(&coord), &r, 7, vec![1]).unwrap())
        } else {
            None
        };
        if cut >= 5 {
            ledger
                .commit(&context(&coord), token.as_ref().unwrap())
                .unwrap();
        }
        if cut >= 6 {
            handler_visible = true;
        }
        let coordinator_image = [
            commit_decision as u8,
            handler_prepared as u8,
            handler_visible as u8,
        ];
        let image = ledger.inner.lock().unwrap().persistence.load().unwrap();
        drop(ledger);
        let commit_decision = coordinator_image[0] != 0;
        let handler_prepared = coordinator_image[1] != 0;
        let mut handler_visible = coordinator_image[2] != 0;
        let ledger = Ledger::open(a, auth.clone(), Memory(image), limits, &context(&read)).unwrap();
        let r = ledger
            .reserve(&context(&coord), key, terms(a, b), Policy::default())
            .unwrap();
        if commit_decision {
            assert!(handler_prepared);
            let token = ledger.prepare(&context(&coord), &r, 7, vec![1]).unwrap();
            ledger.commit(&context(&coord), &token).unwrap();
            ledger.commit(&context(&coord), &token).unwrap();
            handler_visible = true;
        } else {
            let token = if token.is_some() {
                Some(ledger.prepare(&context(&coord), &r, 7, vec![1]).unwrap())
            } else {
                None
            };
            ledger.abort(&context(&coord), &r, token.as_ref()).unwrap();
        }
        assert_eq!(handler_visible, commit_decision);
        assert_eq!(
            ledger.view(&context(&read), b).unwrap().owned,
            if commit_decision { 93 } else { 100 }
        );
        assert_eq!(
            ledger.available(&context(&read), b, true),
            Ok(if commit_decision { 93 } else { 100 })
        );
    }
}

// Original issuer evidence settles each local role once; all forged bindings preserve pending payment.
#[test]
fn issuer_payment_evidence() {
    use super::super::{
        payment::{Payment, Receipt},
        transfer::Order,
        transfer_state::TransferResult,
    };
    let [a, b, c, u] = super::super::test_support::identities();
    for local in [a, b] {
        let auth = Arc::new(Authority::new());
        let admin = auth.grant(local, Action::Administer, 65536).unwrap();
        let read = auth.grant(local, Action::Read, 65536).unwrap();
        let coord = auth.grant(local, Action::Coordinate, 65536).unwrap();
        let evidence = auth.grant(local, Action::Evidence, 65536).unwrap();
        let control = auth.grant(local, Action::Control, 65536).unwrap();
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
            auth.clone(),
            Memory::default(),
            limits,
            &context(&admin),
            100,
            100,
        )
        .unwrap();
        if local == a {
            ledger.set_owned(&context(&admin), c, 100).unwrap();
        } else {
            ledger
                .set_trust(&context(&admin), a, Trust::new(1).unwrap())
                .unwrap();
        }
        let mut t = terms(a, b);
        t.issuer = c;
        t.maximum = 7;
        t.transfer = Some([9; 16]);
        t.fee = Some(2);
        let key = OperationKey {
            requester: a,
            connection: [1; 32],
            stream: 1,
        };
        let r = ledger
            .reserve(
                &context(&coord),
                key,
                t,
                Policy {
                    requester_credits: false,
                    third_party: true,
                },
            )
            .unwrap();
        let token = ledger.prepare(&context(&coord), &r, 7, vec![1]).unwrap();
        ledger.commit(&context(&coord), &token).unwrap();
        assert_eq!(
            ledger
                .payment_request(&context(&coord), key)
                .unwrap()
                .order
                .amount,
            9
        );
        assert_eq!(
            ledger
                .payment_request(&context(&coord), key)
                .unwrap()
                .submit,
            local == a
        );
        assert_eq!(
            ledger.payment(&context(&coord), key),
            Ok(Some(Payment::Pending))
        );
        ledger.payment_problem(&context(&coord), key, true).unwrap();
        assert_eq!(
            ledger.payment(&context(&coord), key),
            Ok(Some(Payment::Unknown))
        );
        ledger
            .payment_problem(&context(&coord), key, false)
            .unwrap();
        assert_eq!(
            ledger.payment(&context(&coord), key),
            Ok(Some(Payment::Unsettled))
        );
        let result = TransferResult {
            order: Order {
                owner: a,
                new_owner: b,
                issuer: c,
                transfer_id: [9; 16],
                amount: 9,
            },
            fee: 2,
        };
        assert!(
            Receipt::authenticated(&context(&control), auth.clone(), local, c, result.clone())
                .is_err()
        );
        for mutation in 0..5 {
            let mut changed = result.clone();
            match mutation {
                0 => changed.order.owner = u,
                1 => changed.order.new_owner = u,
                2 => changed.order.transfer_id = [8; 16],
                3 => changed.order.amount = 10,
                _ => changed.fee = 1,
            };
            let receipt =
                Receipt::authenticated(&context(&evidence), auth.clone(), local, c, changed)
                    .unwrap();
            assert!(
                ledger
                    .confirm_payment(&context(&coord), key, &receipt)
                    .is_err()
            );
        }
        let receipt =
            Receipt::authenticated(&context(&evidence), auth.clone(), local, c, result).unwrap();
        ledger
            .confirm_payment(&context(&coord), key, &receipt)
            .unwrap();
        ledger
            .confirm_payment(&context(&coord), key, &receipt)
            .unwrap();
        assert_eq!(
            ledger.payment(&context(&coord), key),
            Ok(Some(Payment::Settled))
        );
        assert_eq!(
            ledger.view(&context(&read), c).unwrap().owned,
            if local == a { 91 } else { 7 }
        );
        let image = ledger.inner.lock().unwrap().persistence.load().unwrap();
        let reopened =
            Ledger::open(local, auth.clone(), Memory(image), limits, &context(&read)).unwrap();
        assert_eq!(
            reopened.payment(&context(&coord), key),
            Ok(Some(Payment::Settled))
        );
    }
}
