//! Public handler-port substitution and immutable lifetime scenarios.
use super::{
    Error,
    contract::OperationKey,
    handler::*,
    test_support::family::{Echo, offering},
    value::Value,
};
use crate::modules::identity::api::IdentityId;
use std::sync::atomic::Ordering;
// An installed requester and provider independently describe, assess, execute and recover one staged result.
#[test]
fn public_lifecycle_and_checks() {
    let family = Echo::new();
    let a = IdentityId([1; 32]);
    let b = IdentityId([2; 32]);
    let k = OperationKey {
        requester: a,
        connection: [3; 32],
        stream: 1,
    };
    let input = Value::String("hello".into());
    let o = offering(false);
    assert_eq!(
        family.definition().document.value()["service"],
        "org.example.echo"
    );
    assert_eq!(family.terms(&o), Ok(()));
    let assessed = family
        .assess(
            Peer {
                local: b,
                remote: a,
                key: k,
            },
            &o,
            &input,
        )
        .unwrap();
    assert_eq!(assessed.input_bytes, 128);
    assert_eq!(family.executions.load(Ordering::SeqCst), 0);
    let accepted = Blob::new(vec![1], 1).unwrap();
    let i = Invocation {
        key: k,
        requester: a,
        provider: b,
        accepted: accepted.clone(),
        maximum: 7,
        publisher: b,
        issuer: b,
        offering: offering(false),
        definition: family.definition().clone(),
        quote: accepted.clone(),
        factors: Value::Object(vec![]),
        pricing: super::pricing::Price::Fixed(7),
    };
    let c = family.execute(&i, &input).unwrap();
    assert_eq!(c.output, input);
    assert_eq!(c.price, 7);
    assert!(matches!(family.execute(&i, &input), Err(Error::Conflict)));
    assert_eq!(family.completion(k).unwrap().result.bytes(), b"hello");
    let token = family.prepare(k, &c.effect).unwrap();
    assert!(matches!(family.status(k), Ok(Status::Prepared(_))));
    family.commit(k, &token).unwrap();
    family.commit(k, &token).unwrap();
    assert!(matches!(family.status(k), Ok(Status::Committed)));
    assert_eq!(family.abort(k, Some(&token)), Err(Error::Conflict));
    assert_eq!(family.executions.load(Ordering::SeqCst), 1);
    assert_eq!(
        family.verify(&input, &accepted, &c.output, None),
        Ok(Check::Inconclusive)
    );
    assert_eq!(
        family.verify(&input, &accepted, &c.output, Some(&input)),
        Ok(Check::Pass)
    );
    assert_eq!(
        family.verify(
            &input,
            &accepted,
            &Value::String("wrong".into()),
            Some(&input)
        ),
        Ok(Check::Fail)
    );
    assert_eq!(family.evidence(&i, &input), Err(Error::Unsupported));
    let kept = c.result.clone();
    drop(c);
    drop(family);
    assert_eq!(kept.bytes(), b"hello");
}
// Missing work, wrong effect and token, explicit abort and oversized retained bytes fail independently.
#[test]
fn participant_refusals() {
    let f = Echo::new();
    let a = IdentityId([1; 32]);
    let k = OperationKey {
        requester: a,
        connection: [3; 32],
        stream: 2,
    };
    assert!(matches!(Blob::new(vec![1, 2], 1), Err(Error::Limit)));
    assert!(matches!(f.completion(k), Err(Error::Unknown)));
    assert!(matches!(f.status(k), Ok(Status::Absent)));
    let i = Invocation {
        key: k,
        requester: a,
        provider: IdentityId([2; 32]),
        accepted: Blob::new(vec![1], 1).unwrap(),
        maximum: 7,
        publisher: IdentityId([2; 32]),
        issuer: IdentityId([2; 32]),
        offering: offering(false),
        definition: f.definition().clone(),
        quote: Blob::new(vec![1], 1).unwrap(),
        factors: Value::Object(vec![]),
        pricing: super::pricing::Price::Fixed(7),
    };
    assert!(matches!(
        f.execute(&i, &Value::Bool(false)),
        Err(Error::Invalid)
    ));
    let c = f.execute(&i, &Value::String("x".into())).unwrap();
    let wrong = Blob::new(vec![8], 1).unwrap();
    assert!(matches!(f.prepare(k, &wrong), Err(Error::Conflict)));
    let t = f.prepare(k, &c.effect).unwrap();
    assert_eq!(f.abort(k, None), Err(Error::Denied));
    f.abort(k, Some(&t)).unwrap();
    f.abort(k, Some(&t)).unwrap();
    assert_eq!(f.commit(k, &t), Err(Error::Conflict));
    assert!(matches!(f.status(k), Ok(Status::Aborted)));
}
