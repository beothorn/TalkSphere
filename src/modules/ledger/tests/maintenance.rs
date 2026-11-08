//! Capacity and recovery validation preserve obligations rather than evicting history.
use super::super::{
    accounting::Trust,
    authority::{Authority, Grant},
    contract::{Limits, OperationKey},
    persistence::{Memory, Persistence},
    state::State,
    terms::{Policy, Terms},
};
use super::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Grant a bounded caller context independent of any peer metadata.
fn context(g: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant: g,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// Immutable replay records survive maintenance and smaller limits refuse an existing image without rewriting it.
#[test]
fn retained_obligations_and_limits() {
    let [a, b, c, _] = super::super::test_support::identities();
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let coord = auth.grant(a, Action::Coordinate, 65536).unwrap();
    let read = auth.grant(a, Action::Read, 65536).unwrap();
    let limits = Limits {
        accounts: 16,
        operations: 2,
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
    ledger.set_owned(&context(&admin), c, 0).unwrap();
    let terms = Terms {
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
    for stream in 1..=2 {
        ledger
            .reserve(
                &context(&coord),
                OperationKey {
                    requester: a,
                    connection: [1; 32],
                    stream,
                },
                terms.clone(),
                Policy::default(),
            )
            .unwrap();
    }
    assert!(
        ledger
            .reserve(
                &context(&coord),
                OperationKey {
                    requester: a,
                    connection: [1; 32],
                    stream: 3
                },
                terms,
                Policy::default()
            )
            .is_err()
    );
    let usage = ledger.maintain(&context(&admin)).unwrap();
    assert_eq!(usage.operations, 2);
    assert_eq!(usage.owned, 1);
    assert_eq!(ledger.available(&context(&read), b, true), Ok(86));
    let image = ledger.inner.lock().unwrap().persistence.load().unwrap();
    assert!(
        Ledger::open(
            a,
            auth.clone(),
            Memory(image.clone()),
            Limits {
                operations: 1,
                ..limits
            },
            &context(&read)
        )
        .is_err()
    );
    assert_eq!(
        ledger.inner.lock().unwrap().persistence.load().unwrap(),
        image
    );
}
// Every recovered role, effect and capacity association is validated before live state is accepted.
#[test]
fn journal_invariants() {
    use super::super::payment::Payment;
    use super::super::reservation::{Operation, Resource, Status};
    use super::super::terms::Mode;
    let [a, b, c, _] = super::super::test_support::identities();
    let limits = Limits {
        accounts: 8,
        operations: 8,
        transfers: 8,
        claims: 8,
        quotes: 8,
        state_bytes: 65536,
    };
    let key = OperationKey {
        requester: a,
        connection: [1; 32],
        stream: 1,
    };
    let terms = Terms {
        requester: a,
        provider: b,
        issuer: c,
        maximum: 60,
        fixed: false,
        accepted: vec![1],
        transfer: Some([9; 16]),
        fee: Some(2),
        spending_bound: 62,
    };
    let op = Operation {
        terms,
        mode: Mode::ThirdParty,
        resource: Resource::Owned(c),
        held: 9,
        status: Status::Committed,
        effect: Some((7, vec![1])),
        confirmed: false,
        payment: Payment::Pending,
    };
    let mut state = State::default();
    state.accounts.owned.insert(c, 100);
    state.accounts.trust.insert(b, Trust::new(1).unwrap());
    state.operations.insert(key, op.clone());
    let record = state.record(a, limits).unwrap();
    assert!(State::load(&record, a, limits).is_ok());
    for mutation in 0..7 {
        let mut bad = state.clone();
        let op = bad.operations.get_mut(&key).unwrap();
        match mutation {
            0 => op.resource = Resource::Internal,
            1 => op.mode = Mode::Provider,
            2 => op.held = 8,
            3 => op.effect = None,
            4 => op.effect = Some((61, vec![1])),
            5 => op.confirmed = true,
            _ => op.payment = Payment::Settled,
        };
        assert!(bad.record(a, limits).is_err());
    }
}
