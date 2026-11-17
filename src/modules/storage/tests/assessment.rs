//! Independent store dimensions, unresolved byte references and exact pre-upload pricing cases.
use super::*;
use crate::modules::board::{config::Limits, offering::Board};
const B: Budget = Budget {
    depth: 8,
    nodes: 64,
    bytes: 1024,
};
// Parse the pinned complete offering independently of the family assessment implementation.
fn offering() -> Offering {
    let o: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/offering.json")).unwrap();
    let board = serde_json::json!({"version":1,"providedServices":[o],"wantedServices":[]});
    Board::parse(&serde_json::to_vec(&board).unwrap(), Limits::default())
        .unwrap()
        .offerings
        .remove("storage-standard")
        .unwrap()
}
// Supply literal valid metadata and deliberately unverified content; assessment cannot attest to its hash.
fn input() -> Value {
    Value::Object(vec![
        ("operation".into(), Value::String("store".into())),
        ("sizeBytes".into(), Value::Uint64(3)),
        ("retentionSeconds".into(), Value::Uint64(10)),
        ("contentHash".into(), Value::Bytes(vec![9; 32])),
        ("data".into(), Value::Bytes(b"abc".to_vec())),
    ])
}
// Turn only byte fields into unresolved descriptors, without allocating their declared content.
fn described(hash_body: bool) -> Input {
    let Value::Object(fields) = input() else {
        unreachable!()
    };
    Input::Object(
        fields
            .into_iter()
            .map(|(n, v)| {
                let value = if n == "data" {
                    Input::Body {
                        length: 3,
                        text: false,
                    }
                } else if n == "contentHash" && hash_body {
                    Input::Body {
                        length: 32,
                        text: false,
                    }
                } else {
                    Input::Complete(v)
                };
                (n, value)
            })
            .collect(),
    )
}
// Complete, detached data and detached short hashes produce the same local quote without payload placeholders.
#[test]
fn exact_price_and_body_independent_projection() {
    let o = offering();
    let expected = Value::Object(vec![
        ("operation".into(), Value::String("store".into())),
        ("sizeBytes".into(), Value::Uint64(3)),
        ("retentionSeconds".into(), Value::Uint64(10)),
    ]);
    for source in [Input::Complete(input()), described(false), described(true)] {
        let assessed = store(&source, &o, B, 17).unwrap();
        assert_eq!(assessed.price, 17);
        assert_eq!(assessed.assessment.metadata, expected);
        assert_eq!(assessed.assessment.factors, Value::Object(vec![]));
        assert_eq!(assessed.assessment.input_bytes, 89);
        assert!(assessed.assessment.pin.is_none());
        assert_eq!(
            store_price(&o, &expected, &Value::Object(vec![])),
            Ok(Price::Fixed(17))
        );
        assert!(matches!(
            store(&source, &o, B, 16),
            Err(Error::Insufficient)
        ));
    }
    assert_eq!(requester_metadata(&input(), &o, B), Ok(expected));
    // The deliberately wrong content hash above is not validated until complete execution.
}
// Missing, wrong-kind, forbidden and oversized metadata rejects before any body materialization or work.
#[test]
fn invalid_metadata_matrix() {
    let o = offering();
    let Value::Object(fields) = input() else {
        unreachable!()
    };
    for index in 0..fields.len() {
        let mut bad = fields.clone();
        bad.remove(index);
        assert!(store(&Input::Complete(Value::Object(bad)), &o, B, 100).is_err());
        let mut bad = fields.clone();
        bad[index].1 = Value::Bool(false);
        assert!(store(&Input::Complete(Value::Object(bad)), &o, B, 100).is_err());
    }
    for forbidden in [
        "leaseId",
        "offset",
        "length",
        "expectedExpiresAt",
        "additionalSeconds",
    ] {
        let mut bad = fields.clone();
        bad.push((forbidden.into(), Value::Uint64(1)));
        assert!(matches!(
            store(&Input::Complete(Value::Object(bad)), &o, B, 100),
            Err(Error::Invalid)
        ));
    }
    for (name, value) in [
        ("sizeBytes", 0),
        ("sizeBytes", 101),
        ("retentionSeconds", 0),
        ("retentionSeconds", 101),
    ] {
        let mut bad = fields.clone();
        bad.iter_mut().find(|(n, _)| n == name).unwrap().1 = Value::Uint64(value);
        assert!(store(&Input::Complete(Value::Object(bad)), &o, B, 100).is_err());
    }
    for (name, length, text) in [
        ("data", 2, false),
        ("data", 4, false),
        ("data", 3, true),
        ("contentHash", 31, false),
        ("contentHash", 32, true),
    ] {
        let Input::Object(mut f) = described(true) else {
            unreachable!()
        };
        f.iter_mut().find(|(n, _)| n == name).unwrap().1 = Input::Body { length, text };
        assert!(matches!(
            store(&Input::Object(f), &o, B, 100),
            Err(Error::Invalid)
        ));
    }
    let Input::Object(mut f) = described(false) else {
        unreachable!()
    };
    f.iter_mut().find(|(n, _)| n == "data").unwrap().1 = Input::Body {
        length: u64::MAX,
        text: false,
    };
    assert!(matches!(
        store(&Input::Object(f), &o, B, 100),
        Err(Error::Limit)
    ));
    assert!(matches!(
        store(&described(true), &o, Budget { bytes: 88, ..B }, 100),
        Err(Error::Limit)
    ));
    let mut duplicate = fields.clone();
    duplicate.push(fields[0].clone());
    assert!(matches!(
        store(&Input::Complete(Value::Object(duplicate)), &o, B, 100),
        Err(Error::Invalid)
    ));
    let mut future = fields;
    future.push(("future".into(), Value::Bool(true)));
    assert_eq!(
        store(&Input::Complete(Value::Object(future)), &o, B, 100)
            .unwrap()
            .price,
        17
    );
}
// Provider factors cannot substitute requester-known input dimensions or silently inflate a work quote.
#[test]
fn provider_factors_do_not_replace_local_dimensions() {
    let o = offering();
    let meta = requester_metadata(&input(), &o, B).unwrap();
    assert_eq!(
        store_price(
            &o,
            &meta,
            &Value::Object(vec![("sizeBytes".into(), Value::Uint64(100))])
        ),
        Err(Error::Invalid)
    );
    let Value::Object(mut fields) = meta else {
        unreachable!()
    };
    fields.push(("data".into(), Value::Bytes(vec![0])));
    assert!(store_price(&o, &Value::Object(fields), &Value::Object(vec![])).is_err());
    assert!(store_price(&o, &Value::Uint64(1), &Value::Object(vec![])).is_err());
}

#[cfg(target_os = "linux")]
// Recovered capacity refusal precedes upload work, and a provisional probe cannot erase another key's hold.
#[test]
fn recovered_capacity_probe_has_no_upload_effects() {
    use crate::modules::{
        identity::api::IdentityId,
        services::{contract::OperationKey, runtime::Phase},
        storage::{
            bytes::{Store as Bytes, tests::path},
            contract::{Crypto, Limits as Capacity, Time},
            image::Limits as ImageLimits,
            journal::Journal,
            quota::Usage,
            recovery::{Gate, Resume},
            retention::Anchor,
            test_support::ScriptCrypto,
        },
    };
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };
    struct EmptyCoordinator;
    impl Resume for EmptyCoordinator {
        // This test-owned coordinator has no operations and never fabricates a recovery decision.
        fn local_identity(&self) -> IdentityId {
            IdentityId([1; 32])
        }
        // Any unexpected request fails closed instead of authorizing execution or an abort.
        fn resume(&self, _: OperationKey) -> Result<Phase, Error> {
            Err(Error::Unavailable)
        }
    }
    let p = path();
    let bytes = Arc::new(Bytes::open(&p.0, true).unwrap());
    let crypto: Arc<dyn Crypto> = Arc::new(ScriptCrypto {
        ids: Mutex::new(VecDeque::new()),
        hashes: Mutex::new(VecDeque::new()),
    });
    let time = Time {
        unix_seconds: 1000,
        elapsed_seconds: 0,
        boot: [1; 16],
    };
    let image = ImageLimits {
        bytes: 4096,
        leases: 4,
        operations: 4,
        values: B,
    };
    let journal = Arc::new(
        Journal::initialize(
            IdentityId([1; 32]),
            bytes.clone(),
            crypto,
            image,
            Anchor::new(time),
        )
        .unwrap(),
    );
    let capacity = Capacity {
        bytes: 2,
        leases: 4,
        operations: 4,
        result_bytes: 512,
        per_owner: 4,
    };
    let gate = Gate::new(journal.clone(), bytes.clone(), capacity).unwrap();
    let key = OperationKey {
        requester: IdentityId([2; 32]),
        connection: [3; 32],
        stream: 1,
    };
    assert!(matches!(
        admitted_store(&gate, key, &described(true), &offering(), B, 17),
        Err(Error::Unavailable)
    ));
    gate.recover(&EmptyCoordinator, time, None).unwrap();
    assert!(matches!(
        admitted_store(&gate, key, &described(true), &offering(), B, 17),
        Err(Error::Limit)
    ));
    assert_eq!(bytes.inventory(4), Ok(vec![]));
    assert!(journal.snapshot().unwrap().operations.is_empty());
    let gate = Gate::new(
        journal,
        bytes.clone(),
        Capacity {
            bytes: 16,
            ..capacity
        },
    )
    .unwrap();
    gate.recover(&EmptyCoordinator, time, None).unwrap();
    assert_eq!(
        admitted_store(&gate, key, &described(true), &offering(), B, 17)
            .unwrap()
            .price,
        17
    );
    gate.admit(|_, quota| {
        quota.reserve(
            key,
            Usage {
                bytes: 1,
                operations: 1,
                ..Usage::default()
            },
        )
    })
    .unwrap();
    assert!(matches!(
        admitted_store(&gate, key, &described(true), &offering(), B, 17),
        Err(Error::Conflict)
    ));
    assert_eq!(
        gate.admit(|_, quota| quota.probe(
            OperationKey { stream: 2, ..key },
            Usage {
                bytes: 16,
                ..Usage::default()
            }
        )),
        Err(Error::Limit)
    );
    assert_eq!(bytes.inventory(4), Ok(vec![]));
}
