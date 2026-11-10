//! Independent traversal order, provisional ownership and descriptor rejection cases.
use super::{
    Error,
    contract::Limits,
    value::{self, Pending},
};
use crate::modules::services::{handler::Blob, value::Value};
use serde_json::json;
use std::collections::BTreeMap;
// Inserted z,a names encode as a,z and arrays retain index order, assigning IDs0,1,2 without payload data in metadata.
#[test]
fn deterministic_descriptors_and_text() {
    let limits = Limits::default();
    let bytes = vec![7; 4097];
    let input = Value::Object(vec![
        (
            "z".into(),
            Value::Array(vec![
                Value::Bytes(bytes.clone()),
                Value::Bytes(bytes.clone()),
            ]),
        ),
        ("a".into(), Value::Bytes(bytes.clone())),
    ]);
    let encoded = value::encode(&input, limits, false).unwrap();
    assert_eq!(
        encoded.metadata,
        json!({"a":{"$bytes":0,"length":"4097"},"z":[{"$bytes":1,"length":"4097"},{"$bytes":2,"length":"4097"}]})
    );
    let schema = json!({"type":"object","fields":{"a":{"type":"bytes","description":"A."},"z":{"type":"array","description":"Z.","items":{"type":"bytes"}}}});
    let pending = Pending::parse(&encoded.metadata, &schema, limits, false).unwrap();
    assert_eq!(
        pending
            .descriptors()
            .iter()
            .map(|d| d.id)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    let bodies = encoded
        .bodies
        .into_iter()
        .enumerate()
        .map(|(id, b)| (id as u32, b))
        .collect::<BTreeMap<_, _>>();
    let assembled = pending.assemble(&bodies).unwrap();
    let expected = Value::Object(vec![
        ("a".into(), Value::Bytes(bytes.clone())),
        (
            "z".into(),
            Value::Array(vec![Value::Bytes(bytes.clone()), Value::Bytes(bytes)]),
        ),
    ]);
    assert_eq!(assembled, expected);
    let text = Value::String("€".repeat(2000));
    let encoded = value::encode(&text, limits, false).unwrap();
    assert_eq!(encoded.metadata, json!({"$text":0,"length":"6000"}));
    let pending =
        Pending::parse(&encoded.metadata, &json!({"type":"string"}), limits, false).unwrap();
    assert_eq!(
        pending.assemble(&BTreeMap::from([(0, encoded.bodies[0].clone())])),
        Ok(text)
    );
}
// Nonempty small detached bytes are legal; malformed IDs, lengths and wrapper siblings fail before body storage.
#[test]
fn strict_descriptor_admission() {
    let limits = Limits::default();
    let schema = json!({"type":"bytes"});
    for bad in [
        json!({"$bytes":1,"length":"3"}),
        json!({"$bytes":"0","length":"3"}),
        json!({"$bytes":0,"length":"0"}),
        json!({"$bytes":0,"length":"03"}),
        json!({"$bytes":0,"length":3}),
        json!({"$bytes":0,"length":"3","extra":0}),
        json!({"$text":0,"length":"3"}),
    ] {
        assert!(Pending::parse(&bad, &schema, limits, false).is_err());
    }
    let pending =
        Pending::parse(&json!({"$bytes":0,"length":"3"}), &schema, limits, false).unwrap();
    assert!(pending.assemble(&BTreeMap::new()).is_err());
    assert_eq!(
        pending.assemble(&BTreeMap::from([(
            0,
            Blob::new(b"abc".to_vec(), 3).unwrap()
        )])),
        Ok(Value::Bytes(b"abc".to_vec()))
    );
    assert!(
        pending
            .assemble(&BTreeMap::from([(
                0,
                Blob::new(b"ab".to_vec(), 3).unwrap()
            )]))
            .is_err()
    );
    assert!(
        Pending::parse(
            &json!({"$bytes":0,"length":"18446744073709551615"}),
            &schema,
            limits,
            false
        )
        .is_err()
    );
    let array = json!({"type":"array","items":{"type":"bytes"}});
    assert!(
        Pending::parse(
            &json!([{"$bytes":0,"length":"1"},{"$bytes":0,"length":"1"}]),
            &array,
            limits,
            false
        )
        .is_err()
    );
    assert!(
        Pending::parse(
            &json!([{"$bytes":0,"length":"1"},{"$bytes":1,"length":"1"}]),
            &array,
            Limits {
                bodies: 1,
                ..limits
            },
            false
        )
        .is_err()
    );
    assert!(Pending::parse(&json!(false), &json!({"type":"future.type"}), limits, false).is_err());
    assert!(
        value::encode(
            &Value::Bytes(vec![0; 100]),
            Limits {
                state_bytes: 99,
                ..limits
            },
            false
        )
        .is_err()
    );
    let text = Pending::parse(
        &json!({"$text":0,"length":"1"}),
        &json!({"type":"string"}),
        limits,
        false,
    )
    .unwrap();
    assert_eq!(
        text.assemble(&BTreeMap::from([(0, Blob::new(vec![255], 1).unwrap())])),
        Err(Error::Invalid)
    );
}
// Unknown optional fields add no application semantics but their detached transport bytes remain bounded and must complete.
#[test]
fn compatible_extensions_and_inline_values() {
    let schema = json!({"type":"object","fields":{"a":{"type":"uint64","description":"A."},"label":{"type":"string","description":"Optional label.","optional":true}}});
    let metadata = json!({"a":"18446744073709551615","unknown":{"$bytes":0,"length":"1"}});
    let pending = Pending::parse(&metadata, &schema, Limits::default(), false).unwrap();
    assert_eq!(
        pending.assemble(&BTreeMap::from([(0, Blob::new(vec![7], 1).unwrap())])),
        Ok(Value::Object(vec![("a".into(), Value::Uint64(u64::MAX))]))
    );
    assert!(Pending::parse(&json!({"label":"x"}), &schema, Limits::default(), false).is_err());
    let pending = Pending::parse(
        &json!({"$base64":"AP8="}),
        &json!({"type":"bytes"}),
        Limits::default(),
        false,
    )
    .unwrap();
    assert_eq!(
        pending.assemble(&BTreeMap::new()),
        Ok(Value::Bytes(vec![0, 255]))
    );
}
