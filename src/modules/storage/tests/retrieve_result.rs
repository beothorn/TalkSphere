//! Exact result ownership, pre-I/O budgets and original-key local recovery through actual byte/journal stores.
use super::*;
use crate::modules::{
    identity::api::IdentityId,
    storage::{
        contract::{Crypto, State},
        image::{Limits as ImageLimits, Stage},
        retention::Anchor,
        retrieve,
        test_support::{
            contracts::HASH,
            ranges::{fixture, time},
        },
    },
};
use std::sync::Arc;
// Keep the accepted two-byte range result well within finite independent image/scan/value bounds.
fn limits() -> Limits {
    Limits {
        result_bytes: 4096,
        object_bytes: 100,
        values: Budget {
            depth: 8,
            nodes: 128,
            bytes: 4096,
        },
    }
}
// Read an exact typed output field without using the production result builder as an oracle.
fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Value::Object(f) = value else {
        panic!("result object")
    };
    &f.iter().find(|(n, _)| n == name).unwrap().1
}
// Complete results retain original range/hash/price, independent of later lease state and source ownership.
#[test]
fn exact_complete_result_and_resource_accounting() {
    let mut f = fixture();
    let pin = retrieve::accept(
        &f.invocation,
        &f.input,
        &f.image,
        &f.store,
        time(9),
        None,
        limits().values,
    )
    .unwrap();
    assert_eq!(
        reservation(&pin, limits()).unwrap(),
        Usage {
            operations: 1,
            result_bytes: 366,
            ..Usage::default()
        }
    );
    f.image.leases[0].state = State::Removed;
    drop(f.store);
    let c = complete(&pin, limits()).unwrap();
    assert_eq!(c.price, 1);
    assert_eq!(
        field(&c.output, "operation"),
        &Value::String("retrieve".into())
    );
    assert_eq!(field(&c.output, "data"), &Value::Bytes(b"bc".to_vec()));
    assert_eq!(field(&c.output, "offset"), &Value::Uint64(1));
    assert_eq!(
        field(&c.output, "contentHash"),
        &Value::Bytes(HASH.to_vec())
    );
    assert_eq!(field(&c.output, "price"), &Value::Uint64(1));
    assert_eq!(c.result.bytes().len(), 183);
    assert_eq!(
        services::codec::decode(c.result.bytes(), limits().values, 4096).unwrap(),
        c.output
    );
    assert!(matches!(
        Effect::decode(
            c.effect.bytes(),
            IdentityId([1; 32]),
            ImageLimits {
                bytes: 4096,
                leases: 1,
                operations: 1,
                values: limits().values
            }
        ),
        Ok(Effect::NoChange)
    ));
}
// Allocation/shape limits reject before reading even when the backing object is unavailable or corrupted.
#[test]
fn incomplete_corrupt_or_over_budget_never_completes() {
    let f = fixture();
    let pin = retrieve::accept(
        &f.invocation,
        &f.input,
        &f.image,
        &f.store,
        time(1),
        None,
        limits().values,
    )
    .unwrap();
    for cap in [
        Limits {
            result_bytes: 182,
            ..limits()
        },
        Limits {
            object_bytes: 2,
            ..limits()
        },
        Limits {
            values: Budget {
                nodes: 6,
                ..limits().values
            },
            ..limits()
        },
        Limits {
            values: Budget {
                depth: 0,
                ..limits().values
            },
            ..limits()
        },
        Limits {
            values: Budget {
                bytes: 115,
                ..limits().values
            },
            ..limits()
        },
    ] {
        assert!(matches!(complete(&pin, cap), Err(Error::Limit)));
    }
    let object = f.path.0.join(format!("object-{}", "08".repeat(32)));
    std::fs::write(&object, b"abd").unwrap();
    assert!(matches!(complete(&pin, limits()), Err(Error::Storage)));
    std::fs::write(&object, b"a").unwrap();
    assert!(matches!(
        complete(
            &pin,
            Limits {
                result_bytes: 1,
                ..limits()
            }
        ),
        Err(Error::Limit)
    ));
    assert!(matches!(complete(&pin, limits()), Err(Error::Storage)));
}
struct CryptoPort;
impl Crypto for CryptoPort {
    // This local test-only source is used solely for preparation tokens, never peer authentication.
    fn lease_id(&self) -> Result<[u8; 32], Error> {
        Ok([7; 32])
    }
    // Token hashing uses the real public primitive independently of result construction.
    fn hash(&self, bytes: &[u8]) -> Result<[u8; 32], Error> {
        crate::modules::cryptography::primitives::hash::sha256(bytes, bytes.len())
            .map_err(|_| Error::Limit)
    }
}
// Journal recovery retains complete original bytes after the source object disappears and never rereads on local repeats.
#[test]
fn staged_result_survives_source_loss_and_reopen() {
    let f = fixture();
    let pin = retrieve::accept(
        &f.invocation,
        &f.input,
        &f.image,
        &f.store,
        time(1),
        None,
        limits().values,
    )
    .unwrap();
    let store = Arc::new(f.store);
    let il = ImageLimits {
        bytes: 16384,
        leases: 4,
        operations: 8,
        values: limits().values,
    };
    let journal = Journal::initialize(
        IdentityId([1; 32]),
        store.clone(),
        Arc::new(CryptoPort),
        il,
        Anchor::new(time(0)),
    )
    .unwrap();
    let c = stage(&journal, &pin, limits()).unwrap();
    let token = journal.prepare(pin.key(), &c.effect).unwrap();
    journal.commit(pin.key(), &token).unwrap();
    let object = f.path.0.join(format!("object-{}", "08".repeat(32)));
    std::fs::write(&object, b"bad").unwrap();
    assert!(matches!(complete(&pin, limits()), Err(Error::Storage)));
    std::fs::remove_file(f.path.0.join(format!("object-{}", "08".repeat(32)))).unwrap();
    assert!(matches!(
        stage(
            &journal,
            &pin,
            Limits {
                result_bytes: 1,
                ..limits()
            }
        ),
        Err(Error::Limit)
    ));
    let repeated = stage(&journal, &pin, limits()).unwrap();
    assert_eq!(repeated.output, c.output);
    assert!(repeated.result == c.result);
    drop(journal);
    let reopened =
        Journal::open(IdentityId([1; 32]), store.clone(), Arc::new(CryptoPort), il).unwrap();
    assert!(matches!(
        reopened.status(pin.key()),
        Ok(Some(Stage::Committed(_)))
    ));
    let retained = reopened.completion(pin.key()).unwrap();
    assert_eq!(retained.output, c.output);
    assert!(retained.result == c.result);
}

// The internal scan crosses chunk boundaries while returning only the requested range of a larger older lease.
#[test]
fn bounded_scan_captures_cross_chunk_range() {
    use crate::modules::{cryptography::primitives::hash::sha256, services::pricing::Price};
    let mut f = fixture();
    let mut data = vec![b'a'; 65538];
    data[65536] = b'b';
    data[65537] = b'c';
    let hash = sha256(&data, data.len()).unwrap();
    let mut upload = f.store.begin([9; 32], 65538, 65538).unwrap();
    upload.append(0, &data).unwrap();
    drop(upload.finish(hash).unwrap());
    f.image.leases[0].id = [9; 32];
    f.image.leases[0].size_bytes = 65538;
    f.image.leases[0].content_hash = hash;
    let Value::Object(fields) = &mut f.input else {
        unreachable!()
    };
    for (name, value) in fields {
        match name.as_str() {
            "leaseId" => *value = Value::Bytes(vec![9; 32]),
            "offset" => *value = Value::Uint64(65535),
            "length" => *value = Value::Uint64(3),
            _ => {}
        }
    }
    f.invocation.pricing = Price::Fixed(2);
    f.invocation.maximum = 2;
    let pin = retrieve::accept(
        &f.invocation,
        &f.input,
        &f.image,
        &f.store,
        time(1),
        None,
        limits().values,
    )
    .unwrap();
    let c = complete(
        &pin,
        Limits {
            object_bytes: 70000,
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(field(&c.output, "data"), &Value::Bytes(b"abc".to_vec()));
    assert_eq!(c.price, 2);
    assert_eq!(
        field(&c.output, "contentHash"),
        &Value::Bytes(hash.to_vec())
    );
}
