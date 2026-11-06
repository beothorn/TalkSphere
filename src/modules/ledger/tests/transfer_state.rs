//! Durable issuer transfer effects tested independently of quote parsing.
use super::super::{
    authority::{Authority, Grant},
    contract::Limits,
    fees::Accepted,
    persistence::Memory,
};
use super::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Bound a trusted test caller to explicit local authority.
fn context(g: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant: g,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// Accepted order gross9/fee2 commits 91/7 exactly once and survives issuer restart.
#[test]
fn atomic_transfer_and_replay() {
    let [a, b, c, u] = super::super::test_support::identities();
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(c, Action::Administer, 65536).unwrap();
    let read = auth.grant(c, Action::Read, 65536).unwrap();
    let control = auth.grant(c, Action::Control, 65536).unwrap();
    let limits = Limits {
        accounts: 16,
        operations: 16,
        transfers: 1,
        claims: 16,
        quotes: 8,
        state_bytes: 65536,
    };
    let ledger = Ledger::initialize(
        c,
        auth.clone(),
        Memory::default(),
        limits,
        &context(&admin),
        100,
        100,
    )
    .unwrap();
    ledger.grant(&context(&admin), a, 100).unwrap();
    let accepted = Accepted {
        authority: auth.clone(),
        order: Order {
            owner: a,
            new_owner: b,
            issuer: c,
            transfer_id: [1; 16],
            amount: 9,
        },
        fee: 2,
    };
    let result = ledger
        .commit_transfer(&context(&control), &accepted)
        .unwrap();
    assert_eq!(result.received(), Ok(7));
    assert_eq!(
        ledger.commit_transfer(&context(&control), &accepted),
        Ok(result.clone())
    );
    assert_eq!(ledger.view(&context(&read), a).unwrap().issued, 91);
    assert_eq!(ledger.view(&context(&read), b).unwrap().issued, 7);
    let changed = Accepted {
        order: Order {
            amount: 8,
            ..accepted.order.clone()
        },
        ..accepted
    };
    assert_eq!(
        ledger.commit_transfer(&context(&control), &changed),
        Err(Error::Conflict)
    );
    let image = ledger.inner.lock().unwrap().persistence.load().unwrap();
    let reopened = Ledger::open(c, auth.clone(), Memory(image), limits, &context(&read)).unwrap();
    assert_eq!(
        reopened.commit_transfer(
            &context(&control),
            &Accepted {
                authority: auth.clone(),
                order: result.order.clone(),
                fee: 0
            }
        ),
        Ok(result)
    );
    assert!(
        reopened
            .commit_transfer(
                &context(&control),
                &Accepted {
                    authority: auth.clone(),
                    order: Order {
                        owner: a,
                        new_owner: u,
                        issuer: c,
                        transfer_id: [2; 16],
                        amount: 9
                    },
                    fee: 2
                }
            )
            .is_err()
    );
}
