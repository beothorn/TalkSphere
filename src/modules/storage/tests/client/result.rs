//! Contextual ordinary results validate exact original promises before settlement and never alter local catalog snapshots.
use super::*;
use crate::modules::services::handler::Blob;
// Every ordinary result passes independently specified original invocation/input/output expectations without mutating leases.
#[test]
fn all_contextual_results_preserve_catalogue() {
    let f = fixture();
    let (inv, input, output) = store_fixture(false);
    f.client.validate_result(&inv, &input, &output).unwrap();
    let (inv, input, output, lease) = renew_fixture();
    f.client.validate_result(&inv, &input, &output).unwrap();
    assert_eq!(
        f.local.leases.lock().unwrap()[&(inv.key, inv.provider, inv.requester, lease.id)]
            .expires_at,
        1010
    );
    let (inv, input, output, _) = remove_fixture();
    f.client.validate_result(&inv, &input, &output).unwrap();
    let (inv, input, output, lease) = retrieve();
    f.client.validate_result(&inv, &input, &output).unwrap();
    assert_eq!(
        f.local.leases.lock().unwrap()[&(inv.key, inv.provider, inv.requester, lease.id)].state,
        State::Active
    );
}
// Full-object corruption and mismatched original roles/metadata are rejected; dependency loss is never a manufactured success.
#[test]
fn inconsistent_results_and_dependency_failures() {
    let f = fixture();
    let (inv, input, output, _) = retrieve();
    let mut bad = output.clone();
    set(&mut bad, "data", Value::Bytes(b"abd".to_vec()));
    assert_eq!(
        f.client.validate_result(&inv, &input, &bad),
        Err(Error::Invalid)
    );
    let mut changed = inv.clone();
    changed.provider = IdentityId([9; 32]);
    changed.publisher = changed.provider;
    assert_eq!(
        f.client.validate_result(&changed, &input, &output),
        Err(Error::Denied)
    );
    *f.local.fail.lock().unwrap() = Some(Error::Unavailable);
    assert_eq!(
        f.client.validate_result(&inv, &input, &output),
        Err(Error::Unavailable)
    );
    *f.local.fail.lock().unwrap() = None;
    let (inv, input, output) = store_fixture(false);
    *f.clock.0.lock().unwrap() = Err(Error::Unavailable);
    assert_eq!(
        f.client.validate_result(&inv, &input, &output),
        Err(Error::Unavailable)
    );
}
// Partial ranges compare optional private exact expectations, and context-free local checks never claim agreement or whole-object proof.
#[test]
fn partial_expectations_and_private_local_checks() {
    let f = fixture();
    let (mut inv, mut input, mut output, _) = retrieve();
    inv.maximum = 1;
    inv.pricing = crate::modules::services::pricing::Price::Fixed(1);
    set(&mut input, "offset", Value::Uint64(1));
    set(&mut input, "length", Value::Uint64(2));
    set(&mut output, "offset", Value::Uint64(1));
    set(&mut output, "data", Value::Bytes(b"bc".to_vec()));
    set(&mut output, "price", Value::Uint64(1));
    f.local
        .ranges
        .lock()
        .unwrap()
        .insert(inv.key, Blob::new(b"bc".to_vec(), 2).unwrap());
    f.client.validate_result(&inv, &input, &output).unwrap();
    set(&mut output, "data", Value::Bytes(b"bd".to_vec()));
    assert_eq!(
        f.client.validate_result(&inv, &input, &output),
        Err(Error::Invalid)
    );
    assert_eq!(
        f.client.verify(
            &input,
            &inv.accepted,
            &output,
            Some(&Value::Bytes(b"bc".to_vec()))
        ),
        Ok(crate::modules::services::handler::Check::Fail)
    );
    assert_eq!(
        f.client.verify(&input, &inv.accepted, &output, None),
        Ok(crate::modules::services::handler::Check::Inconclusive)
    );
    set(&mut output, "data", Value::Bytes(b"bc".to_vec()));
    assert_eq!(
        f.client.verify(
            &input,
            &inv.accepted,
            &output,
            Some(&Value::Bytes(b"bc".to_vec()))
        ),
        Ok(crate::modules::services::handler::Check::Pass)
    );
}
// Constructor admission rejects unusable local budgets and altered public standard definitions instead of installing remote descriptions as code.
#[test]
fn constructor_and_operation_semantics() {
    let f = fixture();
    let catalogue = Catalogue::new(BoardLimits::default()).unwrap();
    let (inv, input, output) = store_fixture(false);
    assert!(matches!(
        Client::new(
            inv.definition.clone(),
            &catalogue,
            f.local.clone(),
            f.clock.clone(),
            Policy {
                values: Budget { nodes: 0, ..B },
                maximum_clock_difference_seconds: 2
            }
        ),
        Err(Error::Invalid)
    ));
    f.client
        .validate(&inv.definition.document.value()["input"], &input)
        .unwrap();
    f.client
        .validate(&inv.definition.document.value()["output"], &output)
        .unwrap();
    assert!(!f.client.nonfinite());
    assert_eq!(
        f.client.validate(
            &serde_json::json!({"type":"string"}),
            &Value::String("unknown".into())
        ),
        Err(Error::Unsupported)
    );
    let logical = Value::Object(vec![
        ("outcome".into(), Value::String("inconclusive".into())),
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("offset".into(), Value::Uint64(0)),
    ]);
    f.client
        .validate(
            &inv.definition.document.value()["check"]["output"],
            &logical,
        )
        .unwrap();
}

// Descriptor restrictions and private-byte local checks preserve unknown-contract and incomplete expectation failures.
#[test]
fn invalid_logical_conclusions_and_local_verification_limits() {
    let f = fixture();
    let (inv, input, output, _) = retrieve();
    let descriptor = &inv.definition.document.value()["check"]["output"];
    let mut logical = Value::Object(vec![
        ("outcome".into(), Value::String("pass".into())),
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("offset".into(), Value::Uint64(0)),
        ("value".into(), Value::Bytes(vec![1, 2])),
    ]);
    assert_eq!(f.client.validate(descriptor, &logical), Err(Error::Invalid));
    set(&mut logical, "value", Value::Bytes(vec![1]));
    set(&mut logical, "outcome", Value::String("unknown".into()));
    assert_eq!(f.client.validate(descriptor, &logical), Err(Error::Invalid));
    assert_eq!(
        f.client
            .verify(&input, &inv.accepted, &output, Some(&Value::Bytes(vec![1]))),
        Ok(crate::modules::services::handler::Check::Inconclusive)
    );
    let (store, input, output) = store_fixture(false);
    assert_eq!(
        f.client.verify(&input, &store.accepted, &output, None),
        Ok(crate::modules::services::handler::Check::Inconclusive)
    );
    let mut wrong = output;
    set(&mut wrong, "acceptedAt", Value::Uint64(900));
    set(&mut wrong, "expiresAt", Value::Uint64(910));
    assert_eq!(
        f.client.validate_result(&store, &input, &wrong),
        Err(Error::Invalid)
    );
}

// Invalid remote output must be rejected before any private range read/allocation, while a real local expectation failure cannot be silently ignored.
#[test]
fn validate_remote_bounds_before_private_range_dependency() {
    let f = fixture();
    let (inv, mut input, output, _) = retrieve();
    set(&mut input, "length", Value::Uint64(u64::MAX));
    assert!(f.client.validate_result(&inv, &input, &output).is_err());
    assert_eq!(
        f.local
            .range_calls
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    set(&mut input, "length", Value::Uint64(3));
    *f.local.range_error.lock().unwrap() = Some(Error::Unavailable);
    assert_eq!(
        f.client.validate_result(&inv, &input, &output),
        Err(Error::Unavailable)
    );
    assert_eq!(
        f.local
            .range_calls
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}
