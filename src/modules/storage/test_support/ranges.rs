//! Real byte objects and trusted accepted-context substitutes for independent range/result tests.
use crate::modules::{
    services::{
        handler::{Blob, Invocation},
        pricing::Price,
        value::Value,
    },
    storage::{
        bytes::{
            Store,
            tests::{PathGuard, path},
        },
        contract::{Lease, State, Time},
        image::Image,
        retention::Anchor,
        test_support::contracts::{HASH, store as store_fixture},
    },
};
pub(crate) struct Fixture {
    pub path: PathGuard,
    pub store: Store,
    pub image: Image,
    pub invocation: Invocation,
    pub input: Value,
}
// Create actual exact bytes and a test-owned accepted lease; this does not implement blocked paid store creation.
pub(crate) fn fixture() -> Fixture {
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
pub(crate) fn time(elapsed: u64) -> Time {
    Time {
        unix_seconds: 1000 + elapsed,
        elapsed_seconds: elapsed,
        boot: [1; 16],
    }
}
