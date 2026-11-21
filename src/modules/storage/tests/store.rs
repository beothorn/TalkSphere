//! Native immutable uploads and decision-time lease results independent of a network session.
use super::*;
use crate::modules::storage::test_support::{
    contracts,
    mutations::{B, CryptoPort, Terminal, fixture, time},
};

// A prepared upload cannot become active through ordinary commit. Finalization preserves a fresh
// UTC sample and an independent elapsed clock so later UTC corrections cannot shorten retention.
#[test]
fn prepare_and_apply_decision_at_corrected_wall_time() {
    let f = fixture();
    let (mut inv, input, _) = contracts::store(false);
    inv.key.stream = 19;
    let template = stage(&f.gate, &inv, &input, &CryptoPort, B).unwrap();
    validate_template(&inv, &input, &template, f.journal.image_limits()).unwrap();
    let token = f.gate.prepare(inv.key, &template.effect).unwrap();
    assert_eq!(f.gate.commit(inv.key, &token), Err(Error::Conflict));
    assert_eq!(f.journal.snapshot().unwrap().leases.len(), 1);
    let now = Time {
        unix_seconds: 9000,
        ..time(20)
    };
    let finalization = finalize(&f.gate, inv.key, &token, &template, now, None).unwrap();
    assert_eq!(
        super::super::renewal::number(&finalization.output, "acceptedAt"),
        Ok(9000)
    );
    assert_eq!(
        super::super::renewal::number(&finalization.output, "expiresAt"),
        Ok(9010)
    );
    let mut changed = finalization.clone();
    changed.price += 1;
    assert!(
        f.gate
            .commit_store(inv.key, &token, &template, &changed)
            .is_err()
    );
    f.gate
        .commit_store(inv.key, &token, &template, &finalization)
        .unwrap();
    f.journal.reload().unwrap();
    f.gate
        .commit_store(inv.key, &token, &template, &finalization)
        .unwrap();
    let image = f.journal.snapshot().unwrap();
    let lease = image.leases.iter().find(|l| l.id == [7; 32]).unwrap();
    assert_eq!(
        f.bytes.reopen(lease.id, 3).unwrap().read(0, 3, 3).unwrap(),
        b"abc"
    );
    assert!(super::super::retention::active(&image, lease, time(29), None).unwrap());
    assert!(!super::super::retention::active(&image, lease, time(30), None).unwrap());
    f.gate
        .recover(&Terminal(&f.journal), time(30), None)
        .unwrap();
    let image = f.journal.snapshot().unwrap();
    assert_eq!(
        image.leases.iter().find(|l| l.id == [7; 32]).unwrap().state,
        State::Expired
    );
    assert_eq!(
        f.gate.completion(inv.key).unwrap().output,
        finalization.output
    );
}

// Wrong content must release provisional resources. A successful stage is retained even when the
// commit-time expiration overflows, so only an explicit abort may dispose of it.
#[test]
fn invalid_content_and_overflow_never_create_a_lease() {
    let f = fixture();
    let (mut inv, input, _) = contracts::store(false);
    inv.key.stream = 19;
    let mut corrupt = input.clone();
    let Value::Object(fields) = &mut corrupt else {
        panic!("expected fixture object");
    };
    fields
        .iter_mut()
        .find(|(name, _)| name == "data")
        .unwrap()
        .1 = Value::Bytes(b"bad".to_vec());
    assert!(matches!(
        stage(&f.gate, &inv, &corrupt, &CryptoPort, B),
        Err(Error::Invalid)
    ));
    assert!(f.gate.status(inv.key).unwrap().is_none());
    assert!(f.bytes.reopen([7; 32], 3).is_err());
    let template = stage(&f.gate, &inv, &input, &CryptoPort, B).unwrap();
    let token = f.gate.prepare(inv.key, &template.effect).unwrap();
    let now = Time {
        unix_seconds: u64::MAX,
        ..time(20)
    };
    assert!(matches!(
        finalize(&f.gate, inv.key, &token, &template, now, None),
        Err(Error::Limit)
    ));
    f.gate.abort(inv.key, Some(&token)).unwrap();
    assert!(f.bytes.reopen([7; 32], 3).is_err());
    assert_eq!(f.journal.snapshot().unwrap().leases.len(), 1);
}
