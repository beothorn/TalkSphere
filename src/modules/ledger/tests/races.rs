//! Shared source, budget and duplicate-decision races use barriers rather than scheduler sleeps.
use super::*;
use crate::modules::ledger::{
    accounting::Trust,
    authority::{Action, Authority, Grant},
    contract::{Context, Limits, OperationKey},
    fees::Accepted,
    persistence::Memory,
    state::Ledger,
    terms::{Policy, Terms},
    transfer::Order,
};
use std::{
    sync::{Arc, Barrier},
    time::{Duration, Instant},
};
// Positive budgets permit the competing valid requests to reach the same serialization point.
fn limits() -> Limits {
    Limits {
        accounts: 16,
        operations: 16,
        transfers: 16,
        claims: 16,
        quotes: 8,
        state_bytes: 65536,
    }
}
// Borrow explicit caller authority for each raced operation.
fn context(g: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant: g,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// Independently literal accepted terms keep failure attribution on accounting capacity.
fn terms(
    a: crate::modules::identity::api::IdentityId,
    b: crate::modules::identity::api::IdentityId,
) -> Terms {
    Terms {
        requester: a,
        provider: b,
        issuer: b,
        maximum: 60,
        fixed: false,
        accepted: vec![7],
        transfer: None,
        fee: None,
        spending_bound: 100,
    }
}
// Transfers and revocations contend with a provider hold on the same recognized source balance.
#[test]
fn outgoing_sources() {
    let [a, b, c, _] = test_support::identities();
    for transfer in [false, true] {
        let auth = Arc::new(Authority::new());
        let admin = auth.grant(b, Action::Administer, 65536).unwrap();
        let read = auth.grant(b, Action::Read, 65536).unwrap();
        let coord = auth.grant(b, Action::Coordinate, 65536).unwrap();
        let control = auth.grant(b, Action::Control, 65536).unwrap();
        let ledger = Ledger::initialize(
            b,
            auth.clone(),
            Memory::default(),
            limits(),
            &context(&admin),
            100,
            100,
        )
        .unwrap();
        ledger.grant(&context(&admin), a, 100).unwrap();
        ledger
            .set_trust(&context(&admin), a, Trust::new(1).unwrap())
            .unwrap();
        let barrier = Barrier::new(2);
        let (hold, other) = std::thread::scope(|s| {
            let h = s.spawn(|| {
                barrier.wait();
                ledger.reserve(
                    &context(&coord),
                    OperationKey {
                        requester: a,
                        connection: [1; 32],
                        stream: 1,
                    },
                    terms(a, b),
                    Policy::default(),
                )
            });
            let o = s.spawn(|| {
                barrier.wait();
                if transfer {
                    ledger
                        .commit_transfer(
                            &context(&control),
                            &Accepted {
                                authority: auth.clone(),
                                order: Order {
                                    owner: a,
                                    new_owner: c,
                                    issuer: b,
                                    transfer_id: [1; 16],
                                    amount: 60,
                                },
                                fee: 0,
                            },
                        )
                        .map(|_| ())
                } else {
                    ledger.revoke(&context(&admin), a, 60)
                }
            });
            (h.join().unwrap(), o.join().unwrap())
        });
        assert_ne!(hold.is_ok(), other.is_ok());
        assert_eq!(ledger.available(&context(&read), a, false), Ok(40));
    }
}
// A grant and a service issuance hold cannot allocate the same finite issuance capacity.
#[test]
fn issuance_and_commit_replay() {
    let [a, b, _, _] = test_support::identities();
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let read = auth.grant(a, Action::Read, 65536).unwrap();
    let coord = auth.grant(a, Action::Coordinate, 65536).unwrap();
    let ledger = Ledger::initialize(
        a,
        auth.clone(),
        Memory::default(),
        limits(),
        &context(&admin),
        10,
        100,
    )
    .unwrap();
    let mut t = terms(a, b);
    t.issuer = a;
    t.maximum = 7;
    let key = OperationKey {
        requester: a,
        connection: [1; 32],
        stream: 1,
    };
    let barrier = Barrier::new(2);
    let (hold, grant) = std::thread::scope(|s| {
        let h = s.spawn(|| {
            barrier.wait();
            ledger.reserve(&context(&coord), key, t, Policy::default())
        });
        let g = s.spawn(|| {
            barrier.wait();
            ledger.grant(&context(&admin), b, 7)
        });
        (h.join().unwrap(), g.join().unwrap())
    });
    assert_ne!(hold.is_ok(), grant.is_ok());
    assert_eq!(ledger.capacities(&context(&read)).unwrap().0, 3);
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let read = auth.grant(a, Action::Read, 65536).unwrap();
    let coord = auth.grant(a, Action::Coordinate, 65536).unwrap();
    let ledger = Ledger::initialize(
        a,
        auth.clone(),
        Memory::default(),
        limits(),
        &context(&admin),
        100,
        100,
    )
    .unwrap();
    ledger.set_owned(&context(&admin), b, 100).unwrap();
    let r = ledger
        .reserve(&context(&coord), key, terms(a, b), Policy::default())
        .unwrap();
    let p = ledger.prepare(&context(&coord), &r, 21, vec![1]).unwrap();
    let barrier = Barrier::new(2);
    std::thread::scope(|s| {
        let x = s.spawn(|| {
            barrier.wait();
            ledger.commit(&context(&coord), &p)
        });
        let y = s.spawn(|| {
            barrier.wait();
            ledger.commit(&context(&coord), &p)
        });
        x.join().unwrap().unwrap();
        y.join().unwrap().unwrap();
    });
    assert_eq!(ledger.view(&context(&read), b).unwrap().owned, 79);
}
