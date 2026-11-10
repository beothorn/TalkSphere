//! Exact contiguous body ownership and whole-text validation across arbitrary chunk boundaries.
use super::{Error, contract::Limits, receiver::Receiver, record::Record, value::Pending};
use crate::modules::services::value::Value;
use serde_json::json;
// UTF-8 may split across chunks; only a complete exact body produces a logical string.
#[test]
fn ordered_complete_text() {
    let pending = Pending::parse(
        &json!({"$text":0,"length":"3"}),
        &json!({"type":"string"}),
        Limits::default(),
        false,
    )
    .unwrap();
    let mut r = Receiver::new(pending).unwrap();
    assert!(!r.complete());
    for (offset, byte) in "€".as_bytes().iter().enumerate() {
        r.push(&Record::Body {
            id: 0,
            offset: offset as u64,
            data: vec![*byte],
        })
        .unwrap();
    }
    assert!(r.complete());
    assert_eq!(r.finish(), Ok(Value::String("€".into())));
}
// Missing/extra chunks, gaps, overlap, unknown IDs and wrong record kinds permanently invalidate only this value.
#[test]
fn poisoned_body_streams() {
    for bad in [
        Record::Body {
            id: 1,
            offset: 0,
            data: vec![0],
        },
        Record::Body {
            id: 0,
            offset: 1,
            data: vec![0],
        },
        Record::Body {
            id: 0,
            offset: 0,
            data: vec![],
        },
        Record::Body {
            id: 0,
            offset: 0,
            data: vec![0; 4],
        },
        Record::Control(vec![1]),
    ] {
        let pending = Pending::parse(
            &json!({"$bytes":0,"length":"3"}),
            &json!({"type":"bytes"}),
            Limits::default(),
            false,
        )
        .unwrap();
        let mut r = Receiver::new(pending).unwrap();
        assert_eq!(r.push(&bad), Err(Error::Invalid));
        assert!(!r.complete());
        assert_eq!(
            r.push(&Record::Body {
                id: 0,
                offset: 0,
                data: vec![1]
            }),
            Err(Error::Conflict)
        );
        assert_eq!(r.finish(), Err(Error::Invalid));
    }
    let pending = Pending::parse(
        &json!({"$bytes":0,"length":"3"}),
        &json!({"type":"bytes"}),
        Limits::default(),
        false,
    )
    .unwrap();
    let mut r = Receiver::new(pending).unwrap();
    r.push(&Record::Body {
        id: 0,
        offset: 0,
        data: vec![1],
    })
    .unwrap();
    assert_eq!(r.finish(), Err(Error::Invalid));
    let pending = Pending::parse(
        &json!({"$text":0,"length":"1"}),
        &json!({"type":"string"}),
        Limits::default(),
        false,
    )
    .unwrap();
    let mut r = Receiver::new(pending).unwrap();
    assert_eq!(
        r.push(&Record::Body {
            id: 0,
            offset: 0,
            data: vec![255]
        }),
        Err(Error::Invalid)
    );
}
// IDs must finish in order; inline-only values require no body allocation or invented empty record.
#[test]
fn body_order_and_inline() {
    let pending = Pending::parse(
        &json!([{"$bytes":0,"length":"1"},{"$bytes":1,"length":"1"}]),
        &json!({"type":"array","items":{"type":"bytes"}}),
        Limits::default(),
        false,
    )
    .unwrap();
    let mut r = Receiver::new(pending).unwrap();
    r.push(&Record::Body {
        id: 0,
        offset: 0,
        data: vec![3],
    })
    .unwrap();
    assert!(!r.complete());
    r.push(&Record::Body {
        id: 1,
        offset: 0,
        data: vec![4],
    })
    .unwrap();
    assert_eq!(
        r.finish(),
        Ok(Value::Array(vec![
            Value::Bytes(vec![3]),
            Value::Bytes(vec![4])
        ]))
    );
    let pending = Pending::parse(
        &json!({"$base64":""}),
        &json!({"type":"bytes"}),
        Limits::default(),
        false,
    )
    .unwrap();
    let r = Receiver::new(pending).unwrap();
    assert!(r.complete());
    assert_eq!(r.finish(), Ok(Value::Bytes(vec![])));
}
