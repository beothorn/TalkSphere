//! Actual immutable range pins under trusted accepted-context and lease/time substitutes.
use super::*;
use crate::modules::{
    identity::api::IdentityId,
    services::{handler::Blob, value::Value},
    storage::{
        bytes::tests::{PathGuard, path},
        contract::{State, Time},
        retention::Anchor,
        test_support::contracts::{HASH, store as store_fixture},
    },
};
const B: Budget = Budget {
    depth: 8,
    nodes: 128,
    bytes: 4096,
};
struct Fixture {
    path: PathGuard,
    store: Store,
    image: Image,
    invocation: Invocation,
    input: Value,
}
// Create actual exact bytes and a test-owned accepted lease; this does not implement blocked paid store creation.
fn fixture() -> Fixture {
    let path = path();
    let store = Store::open(&path.0, true).unwrap();
    let (mut invocation, _, _) = store_fixture(false);
    let mut upload = store.begin([8; 32], 3, 3).unwrap();
    upload.append(0, b"abc").unwrap();
    drop(upload.finish(HASH).unwrap());
    let lease = Lease {
        id: [8; 32],
        requester: invocation.requester,
        provider: invocation.provider,
        accepted_terms: Blob::new(b"original store".to_vec(), 32).unwrap(),
        size_bytes: 3,
        content_hash: HASH,
        accepted_at: 1000,
        expires_at: 1010,
        store_price: 17,
        state: State::Active,
    };
    let image = Image {
        provider: invocation.provider,
        anchor: Anchor::new(time(0)),
        leases: vec![lease],
        operations: vec![],
    };
    invocation.pricing = Price::Fixed(1);
    invocation.maximum = 1;
    let input = Value::Object(vec![
        ("operation".into(), Value::String("retrieve".into())),
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("offset".into(), Value::Uint64(1)),
        ("length".into(), Value::Uint64(2)),
    ]);
    Fixture {
        path,
        store,
        image,
        invocation,
        input,
    }
}
// Observe a stable provider boot and independently monotonic elapsed seconds.
fn time(elapsed: u64) -> Time {
    Time {
        unix_seconds: 1000 + elapsed,
        elapsed_seconds: elapsed,
        boot: [1; 16],
    }
}
// Change only one typed scenario field when probing range and authority boundaries.
fn set(input: &mut Value, name: &str, value: Value) {
    let Value::Object(f) = input else {
        panic!("fixture object")
    };
    f.iter_mut().find(|(n, _)| n == name).unwrap().1 = value;
}
// A timely accepted subrange remains exact after expiry, metadata removal, publication changes and unlink.
#[test]
fn accepted_pin_survives_later_expiry_and_unlink() {
    let mut f = fixture();
    let pin = accept(
        &f.invocation,
        &f.input,
        &f.image,
        &f.store,
        time(9),
        None,
        B,
    )
    .unwrap();
    assert_eq!(pin.offset(), 1);
    assert_eq!(pin.price(), 1);
    assert_eq!(pin.lease().content_hash, HASH);
    assert_eq!(pin.bytes().len(), 2);
    assert!(!pin.bytes().is_empty());
    assert_eq!(pin.bytes().read(0, 2, 2), Ok(b"bc".to_vec()));
    assert_eq!(pin.bytes().clone().read(1, 1, 1), Ok(b"c".to_vec()));
    assert_eq!(pin.bytes().read(0, 3, 3), Err(Error::Invalid));
    assert_eq!(pin.bytes().read(u64::MAX, 1, 1), Err(Error::Invalid));
    assert_eq!(pin.bytes().read(0, 0, 1), Err(Error::Invalid));
    assert_eq!(pin.bytes().read(0, 2, 1), Err(Error::Limit));
    f.image.leases[0].state = State::Removed;
    assert!(matches!(
        accept(
            &f.invocation,
            &f.input,
            &f.image,
            &f.store,
            time(9),
            None,
            B
        ),
        Err(Error::Unavailable)
    ));
    std::fs::remove_file(f.path.0.join(format!("object-{}", "08".repeat(32)))).unwrap();
    drop(f.store);
    assert_eq!(pin.bytes().read(0, 2, 2), Ok(b"bc".to_vec()));
    assert_eq!(pin.lease().expires_at, 1010);
}
// Exact expiration and malformed/overflow-shaped ranges reject before exposing or allocating bytes.
#[test]
fn range_limits_and_current_offering_prices() {
    let f = fixture();
    assert!(matches!(
        accept(
            &f.invocation,
            &f.input,
            &f.image,
            &f.store,
            time(10),
            None,
            B
        ),
        Err(Error::Unavailable)
    ));
    for (name, value) in [
        ("length", 0),
        ("offset", 3),
        ("offset", u64::MAX),
        ("length", 3),
    ] {
        let mut bad = f.input.clone();
        set(&mut bad, name, Value::Uint64(value));
        assert!(accept(&f.invocation, &bad, &f.image, &f.store, time(1), None, B).is_err());
    }
    let mut current = f.invocation.clone();
    current.offering.source["info"]["maximumRetrieveBytes"] = 1.into();
    assert!(matches!(
        accept(&current, &f.input, &f.image, &f.store, time(1), None, B),
        Err(Error::Limit)
    ));
    current = f.invocation.clone();
    current.offering.source["pricing"]["retrievalByteUsageFactor"]["denominator"] = 1.into();
    current.pricing = Price::Fixed(2);
    current.maximum = 2;
    let pin = accept(&current, &f.input, &f.image, &f.store, time(1), None, B).unwrap();
    assert_eq!(pin.price(), 2);
    assert_eq!(pin.lease().store_price, 17);
    assert_eq!(pin.lease().accepted_terms.bytes(), b"original store");
    let mut whole = f.input.clone();
    set(&mut whole, "offset", Value::Uint64(0));
    set(&mut whole, "length", Value::Uint64(3));
    let mut inv = f.invocation.clone();
    inv.pricing = Price::Fixed(2);
    inv.maximum = 2;
    let pin = accept(&inv, &whole, &f.image, &f.store, time(1), None, B).unwrap();
    assert_eq!(pin.bytes().read(0, 3, 3), Ok(b"abc".to_vec()));
}
// Foreign and missing resources have one denial; original contract and price cannot be replaced by supplied data.
#[test]
fn authority_clock_and_backing_failures() {
    let f = fixture();
    let mut foreign = f.invocation.clone();
    foreign.requester = IdentityId([9; 32]);
    foreign.key.requester = foreign.requester;
    assert!(matches!(
        accept(&foreign, &f.input, &f.image, &f.store, time(1), None, B),
        Err(Error::Denied)
    ));
    let mut missing = f.input.clone();
    set(&mut missing, "leaseId", Value::Bytes(vec![9; 32]));
    assert!(matches!(
        accept(&foreign, &missing, &f.image, &f.store, time(1), None, B),
        Err(Error::Denied)
    ));
    let mut bad = f.invocation.clone();
    bad.maximum = 2;
    assert!(matches!(
        accept(&bad, &f.input, &f.image, &f.store, time(1), None, B),
        Err(Error::Invalid)
    ));
    bad = f.invocation.clone();
    bad.factors = Value::Object(vec![("sizeBytes".into(), Value::Uint64(3))]);
    assert!(matches!(
        accept(&bad, &f.input, &f.image, &f.store, time(1), None, B),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        accept(
            &f.invocation,
            &f.input,
            &f.image,
            &f.store,
            Time {
                boot: [2; 16],
                ..time(1)
            },
            None,
            B
        ),
        Err(Error::Unavailable)
    ));
    let mut corrected = time(1);
    corrected.unix_seconds = 9000;
    assert!(
        accept(
            &f.invocation,
            &f.input,
            &f.image,
            &f.store,
            corrected,
            None,
            B
        )
        .is_ok()
    );
    std::fs::remove_file(f.path.0.join(format!("object-{}", "08".repeat(32)))).unwrap();
    assert!(matches!(
        accept(
            &f.invocation,
            &f.input,
            &f.image,
            &f.store,
            time(1),
            None,
            B
        ),
        Err(Error::Storage)
    ));
}
