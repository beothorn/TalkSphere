//! Actual logical removal decisions preserve accepted immutable reads and deny later operation attempts without replaying success.
use super::*;
use crate::modules::storage::contract::State;
use crate::modules::storage::{
    image::Stage,
    renewal_commit,
    requester::remove,
    test_support::{
        Write,
        contracts::remove as remove_fixture,
        mutations::{B, Terminal, fixture, time},
    },
};
// Successful removal becomes visible only at participant commitment, preserving an already accepted native range handle.
#[test]
fn exact_logical_removal_preserves_accepted_pin() {
    let f = fixture();
    let (inv, input, output, lease) = remove_fixture();
    let mut read = inv.clone();
    read.key.stream = 4;
    read.maximum = 1;
    read.pricing = Price::Fixed(1);
    let read_input = Value::Object(vec![
        ("operation".into(), Value::String("retrieve".into())),
        ("leaseId".into(), Value::Bytes(lease.id.to_vec())),
        ("offset".into(), Value::Uint64(1)),
        ("length".into(), Value::Uint64(2)),
    ]);
    let pin = crate::modules::storage::retrieve::accept(
        &read,
        &read_input,
        &f.journal.snapshot().unwrap(),
        &f.bytes,
        time(1),
        None,
        B,
    )
    .unwrap();
    let completion = stage(&f.gate, &inv, &input, time(1), None, B).unwrap();
    assert_eq!(completion.output, output);
    assert_eq!(completion.price, 2);
    assert_eq!(f.journal.snapshot().unwrap().leases[0].state, State::Active);
    let token = f.gate.prepare(inv.key, &completion.effect).unwrap();
    f.gate.commit(inv.key, &token).unwrap();
    assert_eq!(
        f.journal.snapshot().unwrap().leases[0].state,
        State::Removed
    );
    assert_eq!(pin.bytes().read(0, 2, 2).unwrap(), b"bc");
    assert!(
        crate::modules::storage::retrieve_result::complete(
            &pin,
            crate::modules::storage::retrieve_result::Limits {
                result_bytes: 4096,
                object_bytes: 64,
                values: B
            },
        )
        .is_ok()
    );
    assert!(matches!(
        crate::modules::storage::retrieve::accept(
            &read,
            &read_input,
            &f.journal.snapshot().unwrap(),
            &f.bytes,
            time(1),
            None,
            B,
        ),
        Err(Error::Unavailable)
    ));
    let removed = remove::validate(&inv, &input, &completion.output, &lease, B).unwrap();
    assert_eq!(removed.state, State::Removed);
    // Original local completion is recoverable, whereas a fresh removal invocation is an inactive-lease failure.
    assert_eq!(
        stage(&f.gate, &inv, &input, time(1), None, B)
            .unwrap()
            .output,
        output
    );
    let mut retry = inv.clone();
    retry.key.stream = 4;
    assert!(matches!(
        stage(&f.gate, &retry, &input, time(1), None, B),
        Err(Error::Unavailable)
    ));
    assert!(f.journal.status(retry.key).unwrap().is_none());
    assert!(matches!(
        renewal_commit::stage(&f.gate, &f.invocation, &f.input, time(1), None, B),
        Err(Error::Unavailable)
    ));
    assert_eq!(f.journal.snapshot().unwrap().leases[0].store_price, 17);
}
// Unknown, expired and foreign leases cannot create removal records; price/factor disagreement is rejected before staging.
#[test]
fn inactive_foreign_and_malformed_remove() {
    let f = fixture();
    let (inv, input, _, _) = remove_fixture();
    let mut foreign = inv.clone();
    foreign.requester.0 = [9; 32];
    foreign.key.requester = foreign.requester;
    assert!(matches!(
        stage(&f.gate, &foreign, &input, time(1), None, B),
        Err(Error::Denied)
    ));
    let mut missing = input.clone();
    if let Value::Object(fields) = &mut missing {
        fields.iter_mut().find(|(n, _)| n == "leaseId").unwrap().1 = Value::Bytes(vec![9; 32]);
    }
    assert!(matches!(
        stage(&f.gate, &inv, &missing, time(1), None, B),
        Err(Error::Denied)
    ));
    assert!(matches!(
        stage(&f.gate, &inv, &input, time(10), None, B),
        Err(Error::Unavailable)
    ));
    for index in 0..3 {
        let mut bad = inv.clone();
        match index {
            0 => bad.pricing = Price::Fixed(3),
            1 => bad.maximum = 3,
            _ => bad.factors = Value::Object(vec![("sizeBytes".into(), Value::Uint64(3))]),
        }
        assert!(matches!(
            stage(&f.gate, &bad, &input, time(1), None, B),
            Err(Error::Invalid)
        ));
    }
    assert!(f.journal.status(inv.key).unwrap().is_none());
    assert_eq!(f.journal.snapshot().unwrap().leases[0].state, State::Active);
}
// Confirmed failures leave logical state untouched; lost commit replies are recovered only by repeating the original decision.
#[test]
fn persistence_failure_abort_and_original_decision_recovery() {
    let f = fixture();
    let (inv, input, _, _) = remove_fixture();
    f.faults.writes.lock().unwrap().push_back(Write::Fail);
    assert!(matches!(
        stage(&f.gate, &inv, &input, time(1), None, B),
        Err(Error::Unavailable)
    ));
    let completion = stage(&f.gate, &inv, &input, time(1), None, B).unwrap();
    let token = f.gate.prepare(inv.key, &completion.effect).unwrap();
    f.faults.writes.lock().unwrap().push_back(Write::Fail);
    assert!(matches!(
        f.gate.commit(inv.key, &token),
        Err(Error::Unavailable)
    ));
    assert_eq!(f.journal.snapshot().unwrap().leases[0].state, State::Active);
    f.faults.writes.lock().unwrap().push_back(Write::LoseReply);
    assert!(matches!(
        f.gate.commit(inv.key, &token),
        Err(Error::Unknown)
    ));
    assert!(!f.gate.is_ready().unwrap());
    f.journal.reload().unwrap();
    assert!(matches!(
        f.gate.status(inv.key),
        Ok(Some(Stage::Committed(_)))
    ));
    f.gate.commit(inv.key, &token).unwrap();
    f.gate
        .recover(&Terminal(&f.journal), time(2), None)
        .unwrap();
    assert_eq!(
        f.journal.snapshot().unwrap().leases[0].state,
        State::Removed
    );
    assert!(matches!(
        f.gate.abort(inv.key, Some(&token)),
        Err(Error::Conflict)
    ));
}
// Actual renewal followed by removal observes the new expiry; removal first prevents a later renewal from staging.
#[test]
fn renewed_then_removed_and_removed_then_renewed() {
    for renew_first in [true, false] {
        let f = fixture();
        let (inv, input, _, _) = remove_fixture();
        if renew_first {
            let renewal =
                renewal_commit::stage(&f.gate, &f.invocation, &f.input, time(1), None, B).unwrap();
            let token = f.gate.prepare(f.invocation.key, &renewal.effect).unwrap();
            f.gate.commit(f.invocation.key, &token).unwrap();
        }
        let removal = stage(&f.gate, &inv, &input, time(1), None, B).unwrap();
        let token = f.gate.prepare(inv.key, &removal.effect).unwrap();
        f.gate.commit(inv.key, &token).unwrap();
        let lease = &f.journal.snapshot().unwrap().leases[0];
        assert_eq!(lease.state, State::Removed);
        assert_eq!(lease.expires_at, if renew_first { 1020 } else { 1010 });
        if !renew_first {
            assert!(matches!(
                renewal_commit::stage(&f.gate, &f.invocation, &f.input, time(1), None, B),
                Err(Error::Unavailable)
            ));
        }
    }
}

// Explicit precommit abort preserves the active obligation and saved original status without allowing that attempt to execute again.
#[test]
fn prepared_removal_abort_preserves_active_lease() {
    let f = fixture();
    let (inv, input, _, _) = remove_fixture();
    let completion = stage(&f.gate, &inv, &input, time(1), None, B).unwrap();
    let token = f.gate.prepare(inv.key, &completion.effect).unwrap();
    f.gate.abort(inv.key, Some(&token)).unwrap();
    assert_eq!(f.journal.snapshot().unwrap().leases[0].state, State::Active);
    assert!(matches!(
        stage(&f.gate, &inv, &input, time(1), None, B),
        Err(Error::Conflict)
    ));
    let mut fresh = inv;
    fresh.key.stream = 4;
    assert!(stage(&f.gate, &fresh, &input, time(1), None, B).is_ok());
}
