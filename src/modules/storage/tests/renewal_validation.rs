//! Independent renewal result fixtures check immutable metadata and reject malformed or replayed results.
use super::*;
use crate::modules::storage::test_support::contracts::renew as fixture;
const B: Budget = Budget {
    depth: 8,
    nodes: 128,
    bytes: 4096,
};
// Change one observable logical field independently of the provider implementation.
fn set(value: &mut Value, name: &str, replacement: Value) {
    let Value::Object(f) = value else {
        panic!("fixture object")
    };
    f.iter_mut().find(|(n, _)| n == name).unwrap().1 = replacement;
}
// Successful output extends only expiry; validating it again against an already updated catalog is stale.
#[test]
fn exact_extension_and_stale_result() {
    let (inv, input, output, lease) = fixture();
    let updated = validate(&inv, &input, &output, &lease, B).unwrap();
    assert_eq!(updated.expires_at, 1020);
    assert_eq!(updated.id, lease.id);
    assert_eq!(updated.content_hash, lease.content_hash);
    assert_eq!(updated.accepted_at, lease.accepted_at);
    assert_eq!(updated.store_price, lease.store_price);
    assert_eq!(updated.size_bytes, lease.size_bytes);
    assert!(updated.accepted_terms == lease.accepted_terms);
    assert!(matches!(
        validate(&inv, &input, &output, &updated, B),
        Err(Error::Conflict)
    ));
}
// Missing, wrong-kind, inconsistent expiry/identity/price and incomplete fields cannot update the catalog.
#[test]
fn invalid_result_and_scope_matrix() {
    let (inv, input, output, lease) = fixture();
    let Value::Object(fields) = &output else {
        unreachable!()
    };
    for i in 0..fields.len() {
        let mut missing = fields.clone();
        missing.remove(i);
        assert!(validate(&inv, &input, &Value::Object(missing), &lease, B).is_err());
        let mut bad = output.clone();
        set(&mut bad, &fields[i].0, Value::Bool(false));
        assert!(validate(&inv, &input, &bad, &lease, B).is_err());
    }
    for (name, value) in [
        ("previousExpiresAt", Value::Uint64(1009)),
        ("expiresAt", Value::Uint64(1021)),
        ("price", Value::Uint64(16)),
        ("leaseId", Value::Bytes(vec![9; 32])),
    ] {
        let mut bad = output.clone();
        set(&mut bad, name, value);
        assert!(validate(&inv, &input, &bad, &lease, B).is_err());
    }
    for index in 0..5 {
        let mut bad = inv.clone();
        match index {
            0 => bad.provider.0 = [9; 32],
            1 => bad.requester.0 = [9; 32],
            2 => bad.maximum = 16,
            3 => bad.pricing = Price::Fixed(16),
            _ => bad.factors = Value::Object(vec![]),
        }
        assert!(validate(&bad, &input, &output, &lease, B).is_err());
    }
    assert!(matches!(
        validate(&inv, &input, &output, &lease, Budget { bytes: 1, ..B }),
        Err(Error::Limit)
    ));
}
