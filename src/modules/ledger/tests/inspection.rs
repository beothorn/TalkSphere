//! Public read-only summaries distinguish participant progress from payment resolution.
use super::super::{
    authority::{Authority, Grant},
    contract::{Context, Limits, OperationKey},
    persistence::Memory,
    terms::{Policy, Terms},
};
use super::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Borrow one explicit caller scope for each lifecycle observation.
fn context(g: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant: g,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// Every exposed summary state is observable without exposing terms or secret data.
#[test]
fn lifecycle_summary() {
    let [a, b, c, _] = super::super::test_support::identities();
    let authority = Arc::new(Authority::new());
    let limits = Limits::default();
    let admin = authority
        .grant(a, Action::Administer, limits.state_bytes)
        .unwrap();
    let read = authority
        .grant(a, Action::Read, limits.state_bytes)
        .unwrap();
    let coord = authority
        .grant(a, Action::Coordinate, limits.state_bytes)
        .unwrap();
    let ledger = Ledger::initialize(
        a,
        authority.clone(),
        Memory::default(),
        limits,
        &context(&admin),
        100,
        100,
    )
    .unwrap();
    ledger.set_owned(&context(&admin), b, 100).unwrap();
    ledger.set_owned(&context(&admin), c, 100).unwrap();
    assert!(ledger.summary(&context(&admin)).is_err());
    let terms = Terms {
        requester: a,
        provider: b,
        issuer: b,
        maximum: 7,
        fixed: true,
        accepted: vec![1],
        transfer: None,
        fee: None,
        spending_bound: 9,
    };
    let key = OperationKey {
        requester: a,
        connection: [1; 32],
        stream: 1,
    };
    let r = ledger
        .reserve(&context(&coord), key, terms.clone(), Policy::default())
        .unwrap();
    assert_eq!(ledger.summary(&context(&read)).unwrap().reserved, 1);
    let p = ledger.prepare(&context(&coord), &r, 7, vec![1]).unwrap();
    assert_eq!(ledger.summary(&context(&read)).unwrap().prepared, 1);
    ledger.commit(&context(&coord), &p).unwrap();
    let r = ledger
        .reserve(
            &context(&coord),
            OperationKey { stream: 2, ..key },
            terms.clone(),
            Policy::default(),
        )
        .unwrap();
    ledger.abort(&context(&coord), &r, None).unwrap();
    let key = OperationKey { stream: 3, ..key };
    let r = ledger
        .reserve(
            &context(&coord),
            key,
            Terms {
                issuer: c,
                transfer: Some([9; 16]),
                fee: Some(2),
                ..terms
            },
            Policy {
                requester_credits: false,
                third_party: true,
            },
        )
        .unwrap();
    let p = ledger.prepare(&context(&coord), &r, 7, vec![1]).unwrap();
    ledger.commit(&context(&coord), &p).unwrap();
    let summary = ledger.summary(&context(&read)).unwrap();
    assert_eq!(summary.committed, 2);
    assert_eq!(summary.aborted, 1);
    assert_eq!(summary.pending_payments, 1);
    ledger.payment_problem(&context(&coord), key, true).unwrap();
    assert_eq!(ledger.summary(&context(&read)).unwrap().unknown_payments, 1);
    ledger
        .payment_problem(&context(&coord), key, false)
        .unwrap();
    assert_eq!(
        ledger.summary(&context(&read)).unwrap().unsettled_payments,
        1
    );
}
