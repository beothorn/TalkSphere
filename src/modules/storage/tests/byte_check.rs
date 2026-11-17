//! Actual immutable byte evidence and trusted check-context substitutes enforce price, ownership, boundaries and optional frequency rules.
use super::*;
use crate::modules::{
    services::handler::Blob,
    storage::{
        removal,
        test_support::{
            contracts::remove as remove_fixture,
            mutations::{B, Fixture, fixture, time},
        },
    },
};
// Construct separately paid accepted check context from independent pinned definition/terms, never a store allowance.
fn invocation(f: &Fixture) -> Invocation {
    Invocation {
        key: f.invocation.key,
        requester: f.invocation.requester,
        provider: f.invocation.provider,
        issuer: f.invocation.issuer,
        offering: f.invocation.offering.clone(),
        definition: f.invocation.definition.clone(),
        quote: Blob::new(b"accepted check".to_vec(), 32).unwrap(),
        price: 3,
        response_milliseconds: 5000,
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(60),
    }
}
// Keep expected bytes absent from the challenge's public logical object.
fn challenge(offset: u64) -> Value {
    Value::Object(vec![
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("offset".into(), Value::Uint64(offset)),
    ])
}
// Use recovered admission and an independent stable millisecond clock for original provider acceptance.
fn accept(
    source: &Admission,
    f: &Fixture,
    inv: &Invocation,
    input: &Value,
    elapsed: u64,
    milliseconds: u64,
) -> Result<Pinned, Error> {
    f.gate.admit(|journal, _| {
        source.accept(
            Context {
                invocation: inv,
                image: &journal.snapshot()?,
                bytes: &f.bytes,
                now: time(elapsed),
                reboot_elapsed: None,
                accepted: Stamp {
                    epoch: [1; 32],
                    milliseconds,
                },
            },
            input,
            B,
        )
    })
}
// Evidence contains only the actual byte and original association; later removal/expiry cannot revoke an accepted pin.
#[test]
fn exact_paid_byte_and_pin_lifetime() {
    let f = fixture();
    let inv = invocation(&f);
    let source = Admission::new(Policy::default(), 8).unwrap();
    let pin = accept(&source, &f, &inv, &challenge(1), 1, 1000).unwrap();
    let expected = Value::Object(vec![
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("offset".into(), Value::Uint64(1)),
        ("value".into(), Value::Bytes(vec![b'b'])),
    ]);
    assert_eq!(pin.evidence().unwrap(), expected);
    let (remove, input, _, _) = remove_fixture();
    let c = removal::stage(&f.gate, &remove, &input, time(1), None, B).unwrap();
    let token = f.gate.prepare(remove.key, &c.effect).unwrap();
    f.gate.commit(remove.key, &token).unwrap();
    assert_eq!(pin.evidence().unwrap(), expected);
    assert_eq!(f.gate.collect(time(11), None, 1, None).unwrap().pinned, 1);
    assert!(matches!(
        accept(&source, &f, &inv, &challenge(1), 11, 11000),
        Err(Error::Unavailable)
    ));
    drop(pin);
    assert_eq!(
        f.gate.collect(time(11), None, 1, None).unwrap().reclaimed,
        1
    );
}
// Unknown/foreign/inactive resources, bad offsets, private expected fields and zero/inconsistent check prices cannot produce evidence.
#[test]
fn unauthorized_price_and_shape_failures() {
    let f = fixture();
    let inv = invocation(&f);
    let source = Admission::new(Policy::default(), 8).unwrap();
    let mut foreign = invocation(&f);
    foreign.requester.0 = [9; 32];
    foreign.key.requester = foreign.requester;
    assert!(matches!(
        accept(&source, &f, &foreign, &challenge(1), 1, 1000),
        Err(Error::Denied)
    ));
    let mut missing = challenge(1);
    if let Value::Object(fields) = &mut missing {
        fields[0].1 = Value::Bytes(vec![9; 32]);
    }
    assert!(matches!(
        accept(&source, &f, &inv, &missing, 1, 1000),
        Err(Error::Denied)
    ));
    for offset in [3, u64::MAX] {
        assert!(matches!(
            accept(&source, &f, &inv, &challenge(offset), 1, 1000),
            Err(Error::Invalid)
        ));
    }
    assert!(matches!(
        accept(&source, &f, &inv, &challenge(1), 10, 10000),
        Err(Error::Unavailable)
    ));
    let mut private = challenge(1);
    if let Value::Object(fields) = &mut private {
        fields.push(("expectedByte".into(), Value::Bytes(vec![b'b'])));
    }
    assert!(accept(&source, &f, &inv, &private, 1, 1000).is_err());
    for index in 0..5 {
        let mut bad = invocation(&f);
        match index {
            0 => bad.price = 0,
            1 => bad.price = 17,
            2 => bad.response_milliseconds = 1,
            3 => bad.issuer.0 = [9; 32],
            _ => bad.offering.version = 2,
        }
        assert!(accept(&source, &f, &bad, &challenge(1), 1, 1000).is_err());
    }
}
// Optional overlap refusal is released with the original evidence pin; one-second admission checks exact millisecond boundaries.
#[test]
fn overlap_and_exact_one_second_interval() {
    let f = fixture();
    let inv = invocation(&f);
    let source = Admission::new(
        Policy {
            reject_overlap: true,
            minimum_interval: true,
        },
        8,
    )
    .unwrap();
    let pin = accept(&source, &f, &inv, &challenge(0), 1, 1000).unwrap();
    assert!(matches!(
        accept(&source, &f, &inv, &challenge(0), 2, 2000),
        Err(Error::Unavailable)
    ));
    drop(pin);
    assert!(matches!(
        accept(&source, &f, &inv, &challenge(0), 1, 1999),
        Err(Error::Unavailable)
    ));
    drop(accept(&source, &f, &inv, &challenge(0), 2, 2000).unwrap());
    assert!(matches!(
        accept(&source, &f, &inv, &challenge(0), 2, 1999),
        Err(Error::Invalid)
    ));
    drop(accept(&source, &f, &inv, &challenge(0), 3, 3000).unwrap());
    // Disabling optional rules permits overlapping checks and same-time repeats without introducing hidden stricter timing limits.
    let unrestricted = Admission::new(Policy::default(), 8).unwrap();
    let a = accept(&unrestricted, &f, &inv, &challenge(0), 1, 0).unwrap();
    let b = accept(&unrestricted, &f, &inv, &challenge(2), 1, 0).unwrap();
    assert!(a.evidence().is_ok() && b.evidence().is_ok());
}
// A short backing object produces no evidence; a corrupt byte is returned honestly as byte evidence for requester comparison.
#[test]
fn missing_short_and_changed_byte() {
    let f = fixture();
    let inv = invocation(&f);
    let source = Admission::new(Policy::default(), 8).unwrap();
    let pin = accept(&source, &f, &inv, &challenge(1), 1, 1000).unwrap();
    let path = f.path.0.join(format!("object-{}", "08".repeat(32)));
    std::fs::write(&path, b"adc").unwrap();
    let evidence = pin.evidence().unwrap();
    let Value::Object(fields) = evidence else {
        panic!("evidence object")
    };
    assert_eq!(fields[2].1, Value::Bytes(vec![b'd']));
    std::fs::write(&path, b"a").unwrap();
    assert!(matches!(pin.evidence(), Err(Error::Storage)));
    drop(pin);
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(
        accept(&source, &f, &inv, &challenge(1), 2, 2000),
        Err(Error::Storage)
    ));
}
// Finite tracked lease capacity is independent of prices; changed current accepted check terms never inherit the original store charge.
#[test]
fn finite_capacity_and_new_current_terms() {
    assert!(matches!(
        Admission::new(Policy::default(), 0),
        Err(Error::Invalid)
    ));
    let f = fixture();
    let mut inv = invocation(&f);
    let source = Admission::new(Policy::default(), 1).unwrap();
    drop(accept(&source, &f, &inv, &challenge(0), 1, 1000).unwrap());
    inv.offering.source["checkPrice"] = serde_json::json!(4);
    inv.price = 4;
    assert!(accept(&source, &f, &inv, &challenge(0), 2, 2000).is_ok());
    let mut image = f.journal.snapshot().unwrap();
    let mut second = image.leases[0].clone();
    second.id = [9; 32];
    image.leases.push(second);
    let other = Value::Object(vec![
        ("leaseId".into(), Value::Bytes(vec![9; 32])),
        ("offset".into(), Value::Uint64(0)),
    ]);
    assert!(matches!(
        source.accept(
            Context {
                invocation: &inv,
                image: &image,
                bytes: &f.bytes,
                now: time(2),
                reboot_elapsed: None,
                accepted: Stamp {
                    epoch: [1; 32],
                    milliseconds: 2000
                }
            },
            &other,
            B
        ),
        Err(Error::Limit)
    ));
}
