//! Independent requester retrieval scenarios use original abc bytes and pinned accepted contract fixtures.
use super::*;
use crate::modules::storage::{
    requester::store::{self, Policy},
    test_support::contracts::{HASH, store as store_fixture},
};
const B: Budget = Budget {
    depth: 8,
    nodes: 128,
    bytes: 4096,
};

// Derive retained requester metadata from an independently specified successful store, then select a new retrieval.
fn fixture(whole: bool) -> (Invocation, Value, Value, Lease) {
    let (mut inv, store_input, store_output) = store_fixture(false);
    let lease = store::validate(
        &inv,
        &store_input,
        &store_output,
        B,
        Policy {
            observed_unix_seconds: 1000,
            maximum_clock_difference_seconds: 0,
        },
    )
    .unwrap();
    let price = if whole { 2 } else { 1 };
    inv.pricing = Price::Fixed(price);
    inv.maximum = price;
    let offset = if whole { 0 } else { 1 };
    let input = Value::Object(vec![
        ("operation".into(), Value::String("retrieve".into())),
        ("leaseId".into(), Value::Bytes(lease.id.to_vec())),
        ("offset".into(), Value::Uint64(offset)),
        ("length".into(), Value::Uint64(if whole { 3 } else { 2 })),
    ]);
    let output = Value::Object(vec![
        ("operation".into(), Value::String("retrieve".into())),
        ("leaseId".into(), Value::Bytes(lease.id.to_vec())),
        ("offset".into(), Value::Uint64(offset)),
        (
            "data".into(),
            Value::Bytes(if whole {
                b"abc".to_vec()
            } else {
                b"bc".to_vec()
            }),
        ),
        ("contentHash".into(), Value::Bytes(HASH.to_vec())),
        ("price".into(), Value::Uint64(price)),
    ]);
    (inv, input, output, lease)
}
// Replace a single logical field without depending on the production encoder or provider output builder.
fn set(value: &mut Value, name: &str, replacement: Value) {
    let Value::Object(f) = value else {
        panic!("fixture object")
    };
    f.iter_mut().find(|(n, _)| n == name).unwrap().1 = replacement;
}

// Full objects prove the retained SHA-256; partial ranges explicitly report only the checks actually performed.
#[test]
fn whole_and_partial_integrity_levels() {
    for whole in [false, true] {
        let (inv, input, output, mut lease) = fixture(whole);
        let expected: &[u8] = if whole { b"abc" } else { b"bc" };
        let result = validate(&inv, &input, &output, &lease, Some(expected), B).unwrap();
        assert_eq!(result.data, expected);
        assert_eq!(result.whole_object_hash_checked, whole);
        assert!(result.requester_bytes_compared);
        let metadata_only = validate(&inv, &input, &output, &lease, None, B).unwrap();
        assert!(!metadata_only.requester_bytes_compared);
        assert_eq!(metadata_only.whole_object_hash_checked, whole);
        // Completion is about the originally accepted operation, even after local state changes.
        lease.state = crate::modules::storage::contract::State::Removed;
        lease.expires_at = 0;
        assert!(validate(&inv, &input, &output, &lease, Some(expected), B).is_ok());
        let mut corrupt = output.clone();
        set(
            &mut corrupt,
            "data",
            Value::Bytes(if whole {
                b"abd".to_vec()
            } else {
                b"bd".to_vec()
            }),
        );
        assert_eq!(
            validate(&inv, &input, &corrupt, &lease, Some(expected), B),
            Err(Error::Invalid)
        );
        if whole {
            assert_eq!(
                validate(&inv, &input, &corrupt, &lease, None, B),
                Err(Error::Invalid)
            );
        } else {
            // There is no range proof: matching hash metadata alone cannot authenticate corrupted partial bytes.
            let unchecked = validate(&inv, &input, &corrupt, &lease, None, B).unwrap();
            assert!(!unchecked.whole_object_hash_checked && !unchecked.requester_bytes_compared);
        }
    }
}

// Wrong, incomplete, duplicate or ill-typed output never produces a successful application range.
#[test]
fn complete_output_metadata_and_bounds() {
    let (inv, input, output, lease) = fixture(false);
    let Value::Object(fields) = &output else {
        unreachable!()
    };
    for index in 0..fields.len() {
        let mut missing = fields.clone();
        missing.remove(index);
        assert!(validate(&inv, &input, &Value::Object(missing), &lease, None, B).is_err());
        let mut wrong = output.clone();
        set(&mut wrong, &fields[index].0, Value::Bool(false));
        assert!(validate(&inv, &input, &wrong, &lease, None, B).is_err());
    }
    for (name, v) in [
        ("leaseId", Value::Bytes(vec![9; 32])),
        ("offset", Value::Uint64(0)),
        ("contentHash", Value::Bytes(vec![0; 32])),
        ("price", Value::Uint64(2)),
        ("data", Value::Bytes(vec![b'b'])),
        ("data", Value::Bytes(b"bcd".to_vec())),
        ("operation", Value::String("store".into())),
    ] {
        let mut bad = output.clone();
        set(&mut bad, name, v);
        assert!(validate(&inv, &input, &bad, &lease, None, B).is_err());
    }
    assert_eq!(
        validate(&inv, &input, &output, &lease, Some(b"b"), B),
        Err(Error::Invalid)
    );
    assert_eq!(
        validate(
            &inv,
            &input,
            &output,
            &lease,
            None,
            Budget { bytes: 1, ..B }
        ),
        Err(Error::Limit)
    );
    let mut duplicate = fields.clone();
    duplicate.push(fields[0].clone());
    assert_eq!(
        validate(&inv, &input, &Value::Object(duplicate), &lease, None, B),
        Err(Error::Invalid)
    );
}

// Original roles, current accepted prices, exact factors and subtraction-safe ranges cannot be replaced by peer assertions.
#[test]
fn original_context_and_invalid_ranges() {
    let (inv, input, output, lease) = fixture(false);
    for name in ["leaseId", "offset", "length", "operation"] {
        let mut bad = input.clone();
        let value = match name {
            "leaseId" => Value::Bytes(vec![9; 32]),
            "operation" => Value::String("remove".into()),
            _ => Value::Uint64(u64::MAX),
        };
        set(&mut bad, name, value);
        assert!(validate(&inv, &bad, &output, &lease, None, B).is_err());
    }
    for index in 0..7 {
        let mut bad = inv.clone();
        match index {
            0 => bad.requester.0 = [9; 32],
            1 => bad.provider.0 = [9; 32],
            2 => bad.maximum = 2,
            3 => bad.pricing = Price::Fixed(2),
            4 => bad.factors = Value::Object(vec![("sizeBytes".into(), Value::Uint64(3))]),
            5 => bad.offering.direction = Direction::Wanted,
            _ => bad.offering.version = 2,
        }
        assert!(validate(&bad, &input, &output, &lease, None, B).is_err());
    }
    let mut old = lease.clone();
    old.size_bytes = u64::MAX;
    // Huge sizes are inspected as integers without allocating a huge object for a valid partial range.
    assert!(validate(&inv, &input, &output, &old, Some(b"bc"), B).is_ok());
}

// Saved complete output can be restored after caller loss and must be revalidated against independently retained expectations.
#[test]
fn retained_complete_output_after_caller_loss() {
    let (inv, input, output, lease) = fixture(true);
    let encoded = crate::modules::services::codec::encode(&output, B, 4096).unwrap();
    drop(output);
    let restored = crate::modules::services::codec::decode(&encoded, B, 4096).unwrap();
    let result = validate(&inv, &input, &restored, &lease, Some(b"abc"), B).unwrap();
    assert_eq!(result.data, b"abc");
    assert!(result.whole_object_hash_checked && result.requester_bytes_compared);
}
