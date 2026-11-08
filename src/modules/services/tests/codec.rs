//! Independent local byte fixtures and malformed-input checks; not CR2SE wire vectors.
use super::{
    Error,
    codec::{decode, encode},
    value::{Budget, Value},
};
// Full-width unsigned values and signed zero have literal tagged bytes, not JSON numeric representations.
#[test]
fn literal_local_encoding() {
    let b = Budget {
        depth: 31,
        nodes: 128,
        bytes: 1024,
    };
    assert_eq!(
        encode(&Value::Uint64(u64::MAX), b, 1024),
        Ok(vec![4, 255, 255, 255, 255, 255, 255, 255, 255])
    );
    assert_eq!(
        decode(&[5, 128, 0, 0, 0], b, 1024),
        Ok(Value::Float(0x80000000))
    );
    let v = Value::Array(vec![
        Value::Bool(false),
        Value::Bool(true),
        Value::Int32(i32::MIN),
        Value::Int64(i64::MAX),
        Value::Uint32(u32::MAX),
        Value::Uint64(u64::MAX),
        Value::Float(0x7fc00001),
        Value::Double(0x8000000000000000),
        Value::String("€".into()),
        Value::Bytes(vec![0, 255]),
        Value::Object(vec![("x".into(), Value::Array(vec![]))]),
    ]);
    let bytes = encode(&v, b, 1024).unwrap();
    assert_eq!(decode(&bytes, b, 1024), Ok(v));
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end], b, 1024).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(decode(&trailing, b, 1024), Err(Error::Invalid));
    assert_eq!(decode(&bytes, b, 1), Err(Error::Limit));
    assert_eq!(encode(&Value::Bytes(vec![1; 8]), b, 1), Err(Error::Limit));
}
// Malicious lengths never allocate proportional storage; malformed UTF-8, booleans and duplicate names fail.
#[test]
fn malformed_local_values() {
    let b = Budget {
        depth: 2,
        nodes: 8,
        bytes: 16,
    };
    for bytes in [
        vec![0, 2],
        vec![7, 0, 0, 0, 1, 255],
        vec![8, 255, 255, 255, 255],
        vec![9, 255, 255, 255, 255],
        vec![10, 255, 255, 255, 255],
    ] {
        assert!(decode(&bytes, b, 1024).is_err());
    }
    assert_eq!(decode(&[255], b, 1024), Err(Error::Unsupported));
    assert_eq!(
        decode(&[0, 0], Budget { nodes: 0, ..b }, 1024),
        Err(Error::Limit)
    );
    let duplicate = Value::Object(vec![
        ("x".into(), Value::Bool(false)),
        ("x".into(), Value::Bool(true)),
    ]);
    assert_eq!(encode(&duplicate, b, 1024), Err(Error::Invalid));
    let deep = Value::Array(vec![Value::Array(vec![Value::Array(vec![Value::Bool(
        false,
    )])])]);
    assert_eq!(encode(&deep, b, 1024), Err(Error::Limit));
}
