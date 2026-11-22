//! Once-only issuance of early-release credits under an operation key.
use super::super::{
    authority::{Authority, Grant},
    contract::Limits,
    persistence::Memory,
    state::State,
};
use super::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

// Bounded capacities for the scenarios.
fn limits() -> Limits {
    Limits {
        accounts: 16,
        operations: 4,
        transfers: 16,
        claims: 16,
        quotes: 8,
        state_bytes: 65536,
    }
}
// A caller context for one capability.
fn context(g: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant: g,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// The operation key of removal number `n`.
fn key(n: u8) -> OperationKey {
    OperationKey {
        requester: IdentityId([n; 32]),
        connection: [n; 32],
        stream: u32::from(n),
    }
}
// A ledger for identity 1 with 10 units of issuance, plus its authority and capabilities.
fn setup() -> (Ledger<Memory>, Arc<Authority>, Grant, Grant, Grant) {
    let a = IdentityId([1; 32]);
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let coordinate = auth.grant(a, Action::Coordinate, 65536).unwrap();
    let read = auth.grant(a, Action::Read, 65536).unwrap();
    let ledger = Ledger::initialize(
        a,
        auth.clone(),
        Memory::default(),
        limits(),
        &context(&admin),
        10,
        20,
    )
    .unwrap();
    (ledger, auth, admin, coordinate, read)
}

// The credits are issued once: repeating the exact call changes nothing, a different amount or peer under the same key conflicts,
// and the record survives reopening so recovery after a crash cannot issue them again.
#[test]
fn credits_are_issued_exactly_once_per_operation() {
    let (ledger, auth, _admin, coordinate, read) = setup();
    let b = IdentityId([2; 32]);
    ledger
        .issue_release(&context(&coordinate), key(1), b, 4)
        .unwrap();
    ledger
        .issue_release(&context(&coordinate), key(1), b, 4)
        .unwrap();
    assert_eq!(ledger.view(&context(&read), b).unwrap().issued, 4);
    assert_eq!(ledger.budgets(&context(&read)), Ok((6, 20)));
    assert_eq!(
        ledger.issue_release(&context(&coordinate), key(1), b, 5),
        Err(Error::Conflict)
    );
    assert_eq!(
        ledger.issue_release(&context(&coordinate), key(1), IdentityId([3; 32]), 4),
        Err(Error::Conflict)
    );
    assert_eq!(ledger.release_of(&context(&read), key(1)), Ok(Some((b, 4))));
    assert_eq!(ledger.release_of(&context(&read), key(2)), Ok(None));
    let image = ledger.inner.lock().unwrap().persistence.load().unwrap();
    let reopened = Ledger::open(
        IdentityId([1; 32]),
        auth,
        Memory(image),
        limits(),
        &context(&read),
    )
    .unwrap();
    reopened
        .issue_release(&context(&coordinate), key(1), b, 4)
        .unwrap();
    assert_eq!(reopened.view(&context(&read), b).unwrap().issued, 4);
    assert_eq!(
        reopened.release_of(&context(&read), key(1)),
        Ok(Some((b, 4)))
    );
}

// Invalid requests, missing authority and exhausted capacity refuse without changing balances or recording a key, so a later valid
// call is still possible; capacity is never overdrawn.
#[test]
fn refusals_change_nothing() {
    let (ledger, _auth, admin, coordinate, read) = setup();
    let b = IdentityId([2; 32]);
    assert_eq!(
        ledger.issue_release(&context(&coordinate), key(1), b, 0),
        Err(Error::Invalid)
    );
    assert_eq!(
        ledger.issue_release(&context(&coordinate), key(1), IdentityId([1; 32]), 1),
        Err(Error::Invalid)
    );
    assert_eq!(
        ledger.issue_release(&context(&read), key(1), b, 1),
        Err(Error::Denied)
    );
    assert_eq!(
        ledger.issue_release(&context(&admin), key(1), b, 1),
        Err(Error::Denied),
        "administration is not coordination"
    );
    assert_eq!(
        ledger.issue_release(&context(&coordinate), key(1), b, 11),
        Err(Error::Insufficient)
    );
    assert_eq!(ledger.release_of(&context(&read), key(1)), Ok(None));
    assert_eq!(ledger.budgets(&context(&read)), Ok((10, 20)));
    ledger
        .issue_release(&context(&coordinate), key(1), b, 10)
        .unwrap();
    assert_eq!(
        ledger.issue_release(&context(&coordinate), key(2), b, 1),
        Err(Error::Insufficient)
    );
}

// The set of recorded issuances is bounded like operations; a full set refuses new keys but still answers old ones.
#[test]
fn recorded_issuances_are_bounded() {
    let (ledger, _auth, _admin, coordinate, read) = setup();
    let b = IdentityId([2; 32]);
    for n in 1..=4 {
        ledger
            .issue_release(&context(&coordinate), key(n), b, 1)
            .unwrap();
    }
    assert_eq!(
        ledger.issue_release(&context(&coordinate), key(5), b, 1),
        Err(Error::Limit)
    );
    ledger
        .issue_release(&context(&coordinate), key(4), b, 1)
        .unwrap();
    assert_eq!(ledger.view(&context(&read), b).unwrap().issued, 4);
}

// Corrupt release rows are refused when the state is loaded: wrong width, duplicate keys and too many rows.
#[test]
fn damaged_release_records_are_refused() {
    let (ledger, _auth, _admin, coordinate, _read) = setup();
    let b = IdentityId([2; 32]);
    ledger
        .issue_release(&context(&coordinate), key(1), b, 1)
        .unwrap();
    let image = ledger
        .inner
        .lock()
        .unwrap()
        .persistence
        .load()
        .unwrap()
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&image.payload).unwrap();
    let row = value["releases"][0].clone();
    let load = |v: &serde_json::Value| {
        let mut r = image.clone();
        r.payload = serde_json::to_vec(v).unwrap();
        State::load(&r, IdentityId([1; 32]), limits()).err()
    };
    // The undamaged image loads, so the refusals below come from the damage.
    assert_eq!(load(&value), None);
    value["releases"] = serde_json::json!([row.clone(), row.clone()]);
    assert!(load(&value).is_some(), "duplicate");
    value["releases"] = serde_json::json!([[1, 2]]);
    assert!(load(&value).is_some(), "short row");
    value["releases"] =
        serde_json::json!([row.clone(), row.clone(), row.clone(), row.clone(), row]);
    assert!(load(&value).is_some(), "too many rows");
}
