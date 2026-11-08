//! Finite claim creation and guarded commitment-only persistence.
use super::super::{
    authority::{Authority, Grant},
    contract::Limits,
    persistence::Memory,
};
use super::*;
use crate::modules::cryptography::test_support::ScriptedEntropy;
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};
// One bounded local caller scope for each explicit administrative action.
fn context(g: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant: g,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// Independent requests pin separate random ID/secret bytes and finite retained issuance.
#[test]
fn finite_creation() {
    let [a, b, _, _] = super::super::test_support::identities();
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let read = auth.grant(a, Action::Read, 65536).unwrap();
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
        20,
        10,
    )
    .unwrap();
    let mut entropy = ScriptedEntropy(VecDeque::from([Ok(vec![4; 32]), Ok(vec![5; 32])]));
    let claim = ledger
        .create_claim(&context(&admin), [1; 32], 7, Some(b), &mut entropy)
        .unwrap();
    assert_eq!(claim.id(), [4; 32]);
    assert_eq!(
        ledger.inner.lock().unwrap().state.claims[&[4; 32]]
            .hash
            .as_slice(),
        crate::modules::cryptography::test_support::hex(
            "f849d67325facf04177bc663b2dc544051831c589ef581d412f2eba44834e77c"
        )
    );
    assert_eq!(claim.amount(), 7);
    assert_eq!(
        ledger.claim_creation(&context(&admin), [1; 32]),
        Ok(Some(([4; 32], ClaimState::Available)))
    );
    assert_eq!(ledger.capacities(&context(&read)), Ok((13, 10)));
    assert_eq!(
        ledger.grant(&context(&admin), b, 14),
        Err(Error::Insufficient)
    );
    let image = ledger
        .inner
        .lock()
        .unwrap()
        .persistence
        .load()
        .unwrap()
        .unwrap();
    assert!(!image.payload.windows(32).any(|w| w == [5; 32]));
    let reopened = Ledger::open(
        a,
        auth.clone(),
        Memory(Some(image)),
        limits,
        &context(&read),
    )
    .unwrap();
    assert_eq!(reopened.capacities(&context(&read)), Ok((13, 10)));
    let mut entropy = ScriptedEntropy(VecDeque::from([Ok(vec![6; 32]), Ok(vec![7; 32])]));
    assert!(
        reopened
            .create_claim(&context(&admin), [1; 32], 7, None, &mut entropy)
            .is_err()
    );
    assert!(
        ledger
            .create_claim(&context(&read), [2; 32], 1, None, &mut entropy)
            .is_err()
    );
    assert!(
        ledger
            .create_claim(&context(&admin), [2; 32], 0, None, &mut entropy)
            .is_err()
    );
    assert!(
        ledger
            .create_claim(&context(&admin), [2; 32], 1, None, &mut entropy)
            .is_err()
    );
}
