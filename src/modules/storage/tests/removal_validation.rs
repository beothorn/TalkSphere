//! Independent requester removal outputs cannot claim refunds, change immutable metadata or replace original accepted prices.
use super::*;
use crate::modules::storage::test_support::contracts::remove as fixture;
const B: Budget = Budget {
    depth: 8,
    nodes: 128,
    bytes: 4096,
};
// Change one supplied logical field independently of provider result construction.
fn set(value: &mut Value, name: &str, replacement: Value) {
    let Value::Object(f) = value else {
        panic!("fixture object")
    };
    f.iter_mut().find(|(n, _)| n == name).unwrap().1 = replacement;
}
// Only logical state changes; all original identifiers, prices, content metadata and times remain independently retained.
#[test]
fn exact_terminal_state_preserves_original_obligation_record() {
    let (inv, input, output, lease) = fixture();
    let removed = validate(&inv, &input, &output, &lease, B).unwrap();
    assert_eq!(removed.state, State::Removed);
    assert_eq!(removed.id, lease.id);
    assert_eq!(removed.store_price, 17);
    assert_eq!(removed.expires_at, lease.expires_at);
    assert_eq!(removed.accepted_at, lease.accepted_at);
    assert_eq!(removed.content_hash, lease.content_hash);
    assert_eq!(removed.requester, lease.requester);
    assert_eq!(removed.provider, lease.provider);
    assert!(removed.accepted_terms == lease.accepted_terms);
}
// Missing, false, wrong-kind or inconsistent success fields never update requester catalog state.
#[test]
fn malformed_removed_identity_and_price() {
    let (inv, input, output, lease) = fixture();
    let Value::Object(fields) = &output else {
        unreachable!()
    };
    for i in 0..fields.len() {
        let mut missing = fields.clone();
        missing.remove(i);
        assert!(validate(&inv, &input, &Value::Object(missing), &lease, B).is_err());
        let mut bad = output.clone();
        set(&mut bad, &fields[i].0, Value::Uint64(0));
        assert!(validate(&inv, &input, &bad, &lease, B).is_err());
    }
    for (name, value) in [
        ("removed", Value::Bool(false)),
        ("price", Value::Uint64(3)),
        ("leaseId", Value::Bytes(vec![9; 32])),
    ] {
        let mut bad = output.clone();
        set(&mut bad, name, value);
        assert!(validate(&inv, &input, &bad, &lease, B).is_err());
    }
    let mut wrong = input.clone();
    set(&mut wrong, "leaseId", Value::Bytes(vec![9; 32]));
    assert!(matches!(
        validate(&inv, &wrong, &output, &lease, B),
        Err(Error::Invalid)
    ));
    for index in 0..5 {
        let mut bad = inv.clone();
        match index {
            0 => bad.provider.0 = [9; 32],
            1 => bad.requester.0 = [9; 32],
            2 => bad.maximum = 3,
            3 => bad.pricing = Price::Fixed(3),
            _ => bad.factors = Value::Object(vec![("sizeBytes".into(), Value::Uint64(3))]),
        }
        assert!(validate(&bad, &input, &output, &lease, B).is_err());
    }
    assert!(matches!(
        validate(&inv, &input, &output, &lease, Budget { bytes: 1, ..B }),
        Err(Error::Limit)
    ));
}
