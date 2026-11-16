//! Bounded owner-image exactness, corruption and actual byte recovery cases.

use super::*;
use crate::modules::services::value::Value;
// Use small independent budgets while preserving every exact integer extremum in retained values.
fn limits() -> Limits {
    Limits {
        bytes: 4096,
        leases: 4,
        operations: 4,
        values: Budget {
            bytes: 512,
            nodes: 32,
            depth: 8,
        },
    }
}
// Construct owner records independently from metadata decoding and with a complete original operation.
fn image() -> Image {
    let provider = IdentityId([1; 32]);
    let requester = IdentityId([2; 32]);
    Image {
        provider,
        anchor: Anchor::new(Time {
            unix_seconds: u64::MAX,
            elapsed_seconds: 123,
            boot: [9; 16],
        }),
        leases: vec![Lease {
            id: [3; 32],
            requester,
            provider,
            accepted_terms: Blob::new(b"accepted".to_vec(), 8).unwrap(),
            size_bytes: 3,
            content_hash: [4; 32],
            accepted_at: 1000,
            expires_at: u64::MAX,
            store_price: u64::MAX,
            state: State::Active,
        }],
        operations: vec![Retained {
            key: OperationKey {
                requester,
                connection: [5; 32],
                stream: u32::MAX,
            },
            binding: [6; 32],
            completion: Completion {
                output: Value::Object(vec![("exact".into(), Value::Uint64(u64::MAX))]),
                result: Blob::new(b"result".to_vec(), 6).unwrap(),
                effect: Blob::new(b"effect".to_vec(), 6).unwrap(),
                price: u64::MAX,
            },
            status: Stage::Staged,
        }],
    }
}
// Compare opaque preparation tokens explicitly without exposing a debugging representation of retained bytes.
fn same_stage(left: &Stage, right: &Stage) -> bool {
    match (left, right) {
        (Stage::Staged, Stage::Staged) | (Stage::Aborted(None), Stage::Aborted(None)) => true,
        (Stage::Prepared(a), Stage::Prepared(b))
        | (Stage::Committed(a), Stage::Committed(b))
        | (Stage::Aborted(Some(a)), Stage::Aborted(Some(b))) => a == b,
        _ => false,
    }
}
// Every participant status and terminal lease state round-trips with original terms, bytes and widths intact.
#[test]
fn complete_image_roundtrips() {
    for status in [
        Stage::Staged,
        Stage::Prepared(Blob::new(vec![7; 32], 32).unwrap()),
        Stage::Committed(Blob::new(vec![8; 32], 32).unwrap()),
        Stage::Aborted(None),
        Stage::Aborted(Some(Blob::new(vec![9; 32], 32).unwrap())),
    ] {
        for state in [State::Active, State::Expired, State::Removed] {
            let mut original = image();
            original.operations[0].status = status.clone();
            original.leases[0].state = state;
            let bytes = original.encode(limits()).unwrap();
            assert_eq!(&bytes[..8], b"TSSTORE1");
            let decoded = Image::decode(&bytes, original.provider, limits()).unwrap();
            assert_eq!(decoded.encode(limits()).unwrap(), bytes);
            assert_eq!(decoded.leases[0].store_price, u64::MAX);
            assert_eq!(decoded.leases[0].expires_at, u64::MAX);
            assert_eq!(decoded.leases[0].accepted_terms.bytes(), b"accepted");
            assert_eq!(decoded.operations[0].key.stream, u32::MAX);
            assert_eq!(
                decoded.operations[0].completion.output,
                original.operations[0].completion.output
            );
            assert!(same_stage(&decoded.operations[0].status, &status));
        }
    }
}
// Truncation, byte corruption, unknown versions and extra bytes cannot become silently fresh state.
#[test]
fn corruption_owner_and_format_bounds() {
    let original = image();
    let bytes = original.encode(limits()).unwrap();
    for end in 0..bytes.len() {
        assert!(Image::decode(&bytes[..end], original.provider, limits()).is_err());
    }
    for index in 0..bytes.len() {
        let mut bad = bytes.clone();
        bad[index] ^= 1;
        assert!(Image::decode(&bad, original.provider, limits()).is_err());
    }
    assert!(matches!(
        Image::decode(&bytes, IdentityId([8; 32]), limits()),
        Err(Error::Denied)
    ));
    assert!(matches!(
        Image::decode(
            &bytes,
            original.provider,
            Limits {
                bytes: 128,
                ..limits()
            }
        ),
        Err(Error::Limit)
    ));
    assert!(matches!(
        original.encode(Limits {
            bytes: 128,
            ..limits()
        }),
        Err(Error::Limit)
    ));
    let mut future = bytes.clone();
    future[7] = b'2';
    let end = future.len() - 32;
    let digest = sha256(&future[..end], 4096).unwrap();
    future[end..].copy_from_slice(&digest);
    assert!(matches!(
        Image::decode(&future, original.provider, limits()),
        Err(Error::Invalid)
    ));
    let mut extra = bytes[..bytes.len() - 32].to_vec();
    extra.push(0);
    let digest = sha256(&extra, 4096).unwrap();
    extra.extend_from_slice(&digest);
    assert!(matches!(
        Image::decode(&extra, original.provider, limits()),
        Err(Error::Invalid)
    ));
}
// Valid checksums do not make oversized collection counts or missing fields structurally valid.
#[test]
fn checked_framing_before_allocation() {
    let original = image();
    let mut count = original.encode(limits()).unwrap();
    count[80..84].copy_from_slice(&u32::MAX.to_be_bytes());
    let end = count.len() - 32;
    let hash = sha256(&count[..end], 4096).unwrap();
    count[end..].copy_from_slice(&hash);
    assert!(matches!(
        Image::decode(&count, original.provider, limits()),
        Err(Error::Limit)
    ));
    let mut short = MAGIC.to_vec();
    short.extend_from_slice(&original.provider.0);
    let hash = sha256(&short, 4096).unwrap();
    short.extend_from_slice(&hash);
    assert!(matches!(
        Image::decode(&short, original.provider, limits()),
        Err(Error::Invalid)
    ));
    let mut bad_stage = original.encode(limits()).unwrap();
    let end = bad_stage.len() - 32;
    bad_stage[end - 1] = 4;
    let hash = sha256(&bad_stage[..end], 4096).unwrap();
    bad_stage[end..].copy_from_slice(&hash);
    assert!(matches!(
        Image::decode(&bad_stage, original.provider, limits()),
        Err(Error::Invalid)
    ));
}
// Duplicate IDs/keys, foreign provider state and inconsistent leases fail before durable encoding.
#[test]
fn invalid_owned_records() {
    let mut x = image();
    x.leases.push(x.leases[0].clone());
    assert!(matches!(x.encode(limits()), Err(Error::Invalid)));
    let mut x = image();
    x.operations.push(x.operations[0].clone());
    assert!(matches!(x.encode(limits()), Err(Error::Invalid)));
    for field in 0..6 {
        let mut x = image();
        match field {
            0 => x.leases[0].size_bytes = 0,
            1 => x.leases[0].expires_at = 1000,
            2 => x.leases[0].store_price = 0,
            3 => x.leases[0].state = State::Receiving,
            4 => x.leases[0].provider = IdentityId([8; 32]),
            _ => x.leases[0].accepted_terms = Blob::new(vec![], 1).unwrap(),
        };
        assert!(matches!(x.encode(limits()), Err(Error::Invalid)));
    }
    let mut x = image();
    x.operations[0].completion.price = 0;
    assert!(matches!(x.encode(limits()), Err(Error::Invalid)));
    let mut x = image();
    x.operations[0].status = Stage::Prepared(Blob::new(vec![], 1).unwrap());
    assert!(matches!(x.encode(limits()), Err(Error::Invalid)));
    assert!(matches!(
        image().encode(Limits {
            leases: 0,
            ..limits()
        }),
        Err(Error::Invalid)
    ));
}
#[cfg(target_os = "linux")]
// Actual durable metadata and byte objects reopen together, while missing/corrupt active bytes gate recovery.
#[test]
fn metadata_and_object_restart() {
    use super::super::{
        bytes::{Store, tests::path},
        contract::Persistence,
    };
    let p = path();
    let store = Store::open(&p.0, true).unwrap();
    let mut x = image();
    let hash = sha256(b"abc", 3).unwrap();
    x.leases[0].content_hash = hash;
    let mut upload = store.begin([3; 32], 3, 3).unwrap();
    upload.append(0, b"abc").unwrap();
    drop(upload.finish(hash).unwrap());
    store.replace(&x.encode(limits()).unwrap()).unwrap();
    drop(store);
    let store = Store::open(&p.0, false).unwrap();
    let mut recovered =
        Image::decode(&store.load(4096).unwrap().unwrap(), x.provider, limits()).unwrap();
    recovered.verify_active(&store).unwrap();
    recovered.leases[0].content_hash = [0; 32];
    assert_eq!(recovered.verify_active(&store), Err(Error::Storage));
    recovered.leases[0].id = [4; 32];
    assert_eq!(recovered.verify_active(&store), Err(Error::Storage));
    recovered.leases[0].state = State::Expired;
    assert_eq!(recovered.verify_active(&store), Ok(()));
}
