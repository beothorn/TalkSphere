//! Exact scalar vectors and independently invalid numeric/IEEE representations.
use super::{Error, scalar};
use crate::modules::services::value::Value;
use serde_json::json;
// Unsigned extrema, signed widths and IEEE bit objects use the pinned wire representation, never JSON f64 coercion.
#[test]
fn scalar_vectors() {
    for (kind, value, wire) in [
        ("bool", Value::Bool(true), json!(true)),
        ("string", Value::String("€".into()), json!("€")),
        ("int32", Value::Int32(i32::MIN), json!(-2147483648i64)),
        ("uint32", Value::Uint32(u32::MAX), json!(4294967295u64)),
        (
            "int64",
            Value::Int64(i64::MIN),
            json!("-9223372036854775808"),
        ),
        ("uint64", Value::Uint64(0), json!("0")),
        (
            "uint64",
            Value::Uint64(9007199254740993),
            json!("9007199254740993"),
        ),
        (
            "uint64",
            Value::Uint64(u64::MAX),
            json!("18446744073709551615"),
        ),
        (
            "float",
            Value::Float(0x80000000),
            json!({"$float":"80000000"}),
        ),
        (
            "double",
            Value::Double(0x3ff0000000000000),
            json!({"$double":"3ff0000000000000"}),
        ),
    ] {
        assert_eq!(scalar::decode(kind, &wire, false), Ok(value.clone()));
        assert_eq!(scalar::encode(&value, false), Ok(wire));
    }
    for wire in [
        json!(1),
        json!("01"),
        json!("+1"),
        json!("-0"),
        json!("1e2"),
        json!("1.0"),
        json!(null),
        json!("18446744073709551616"),
    ] {
        assert_eq!(scalar::decode("uint64", &wire, false), Err(Error::Invalid));
    }
    for wire in [json!("+1"), json!("-0"), json!("-9223372036854775809")] {
        assert_eq!(scalar::decode("int64", &wire, false), Err(Error::Invalid));
    }
    for wire in [json!(-1), json!(1.0), json!(4294967296u64)] {
        assert_eq!(scalar::decode("uint32", &wire, false), Err(Error::Invalid));
    }
    for wire in [json!(2147483648i64), json!(-2147483649i64), json!(1.5)] {
        assert_eq!(scalar::decode("int32", &wire, false), Err(Error::Invalid));
    }
    assert_eq!(
        scalar::decode("object", &json!({}), false),
        Err(Error::Unsupported)
    );
    assert_eq!(
        scalar::encode(&Value::Bytes(vec![]), false),
        Err(Error::Unsupported)
    );
}
// Only trusted contracts permit exceptional floats, and each NaN has one canonical wire bit pattern.
#[test]
fn canonical_nonfinite() {
    for (kind, key, text, value) in [
        ("float", "$float", "7fc00000", Value::Float(0x7fc00000)),
        (
            "double",
            "$double",
            "7ff8000000000000",
            Value::Double(0x7ff8000000000000),
        ),
    ] {
        let wire = json!({key:text});
        assert_eq!(scalar::decode(kind, &wire, false), Err(Error::Invalid));
        assert_eq!(scalar::decode(kind, &wire, true), Ok(value.clone()));
        assert_eq!(scalar::encode(&value, false), Err(Error::Invalid));
        assert_eq!(scalar::encode(&value, true), Ok(wire));
    }
    assert_eq!(
        scalar::encode(&Value::Float(0x7fc00001), true),
        Ok(json!({"$float":"7fc00000"}))
    );
    assert_eq!(
        scalar::encode(&Value::Double(0xfff8000000000001), true),
        Ok(json!({"$double":"7ff8000000000000"}))
    );
    for wire in [
        json!({"$float":"7fc00001"}),
        json!({"$float":"7FC00000"}),
        json!({"$float":"0000"}),
        json!({"$float":"80000000","extra":1}),
    ] {
        assert_eq!(scalar::decode("float", &wire, true), Err(Error::Invalid));
    }
    assert_eq!(
        scalar::decode("float", &json!({"$float":"7f800000"}), false),
        Err(Error::Invalid)
    );
    assert_eq!(
        scalar::decode("float", &json!({"$float":"7f800000"}), true),
        Ok(Value::Float(0x7f800000))
    );
}
