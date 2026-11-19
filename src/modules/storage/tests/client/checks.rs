//! Separately paid Storage Rules use provider-scoped original leases and keep expected bytes out of public challenges.
use super::*;
use crate::modules::services::{
    checks::{self, Mode, Rules, Timing},
    handler::{Blob, Check},
};
// Construct original trusted check context without changing the accepted ordinary lease fixture or choosing store timestamps.
fn invocation() -> checks::Invocation {
    let (inv, _, _, _) = renew_fixture();
    checks::Invocation {
        key: inv.key,
        requester: inv.requester,
        provider: inv.provider,
        issuer: inv.issuer,
        offering: inv.offering,
        definition: inv.definition,
        quote: Blob::new(b"accepted check".to_vec(), 32).unwrap(),
        price: 3,
        response_milliseconds: 5000,
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(60),
    }
}
// Public challenge contains no expected value or provider-controlled pass assertion.
fn challenge() -> Value {
    Value::Object(vec![
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("offset".into(), Value::Uint64(1)),
    ])
}
// Complete evidence independently names original lease/offset and only one observed byte.
fn evidence(byte: u8) -> Value {
    Value::Object(vec![
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("offset".into(), Value::Uint64(1)),
        ("value".into(), Value::Bytes(vec![byte])),
    ])
}
// Expected data is a separate requester-owned shape, never an inferred provider field or included store entitlement.
fn expectations() -> Value {
    Value::Object(vec![
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("offset".into(), Value::Uint64(1)),
        ("expectedByte".into(), Value::Bytes(vec![b'b'])),
        ("active".into(), Value::Bool(true)),
    ])
}
// The real installed requester declares paid mode and exact offered timing, then validates original ownership and one-byte evidence.
#[test]
fn paid_terms_original_scope_and_evidence_shape() {
    let f = fixture();
    let inv = invocation();
    assert!(matches!(
        f.client.mode(&inv.offering, &challenge()),
        Ok(Mode::Paid)
    ));
    assert_eq!(f.client.response_milliseconds(&inv.offering), Ok(5000));
    f.client.challenge(&inv, &challenge()).unwrap();
    f.client.evidence(&challenge(), &evidence(b'b')).unwrap();
    let schema = f.client.evidence_schema().unwrap();
    assert_eq!(schema["fields"].as_object().unwrap().len(), 3);
    assert!(schema["fields"].get("outcome").is_none());
    let mut wrong = challenge();
    set(&mut wrong, "offset", Value::Uint64(2));
    assert_eq!(
        f.client.evidence(&wrong, &evidence(b'b')),
        Err(Error::Invalid)
    );
    let mut private = challenge();
    if let Value::Object(fields) = &mut private {
        fields.push(("expectedByte".into(), Value::Bytes(vec![b'b'])));
    }
    assert_eq!(f.client.shape(&private), Err(Error::Invalid));
}
// Trusted original within-interval timing and private independent expectations are both necessary; wrong bytes fail without refunds or trust changes.
#[test]
fn independent_private_outcomes_and_ambiguous_timing() {
    let f = fixture();
    let rules = f.client.checks().unwrap();
    assert_eq!(
        rules.outcome(
            &challenge(),
            Some(&evidence(b'b')),
            Some(&expectations()),
            Timing::Within(5000)
        ),
        Check::Pass
    );
    assert_eq!(
        rules.outcome(
            &challenge(),
            Some(&evidence(b'c')),
            Some(&expectations()),
            Timing::Within(5000)
        ),
        Check::Fail
    );
    for timing in [Timing::Ambiguous, Timing::Missing] {
        assert_eq!(
            rules.outcome(
                &challenge(),
                Some(&evidence(b'b')),
                Some(&expectations()),
                timing
            ),
            Check::Inconclusive
        );
    }
    assert_eq!(
        rules.outcome(&challenge(), Some(&evidence(b'b')), None, Timing::Within(1)),
        Check::Inconclusive
    );
    assert_eq!(
        rules.outcome(&challenge(), None, Some(&expectations()), Timing::Within(1)),
        Check::Inconclusive
    );
    assert_eq!(
        rules.outcome(
            &challenge(),
            Some(&evidence(b'b')),
            Some(&evidence(b'b')),
            Timing::Within(1)
        ),
        Check::Inconclusive
    );
    let mut ambiguous = expectations();
    set(&mut ambiguous, "active", Value::Bool(false));
    assert_eq!(
        rules.outcome(
            &challenge(),
            Some(&evidence(b'b')),
            Some(&ambiguous),
            Timing::Within(1)
        ),
        Check::Inconclusive
    );
}

// Wrong owner, inactive snapshots, invalid offsets and unrepresentable local response windows never become paid check admission.
#[test]
fn invalid_checks_and_private_expectation_shapes() {
    let f = fixture();
    let inv = invocation();
    let mut bad = challenge();
    set(&mut bad, "offset", Value::Uint64(3));
    assert_eq!(f.client.challenge(&inv, &bad), Err(Error::Invalid));
    let mut private = evidence(b'b');
    if let Value::Object(fields) = &mut private {
        fields.push(("outcome".into(), Value::String("pass".into())));
    }
    assert_eq!(
        f.client.evidence(&challenge(), &private),
        Err(Error::Invalid)
    );
    let mut offering = inv.offering.clone();
    offering.source["info"]["checkResponseSeconds"] = serde_json::json!(u64::MAX);
    assert_eq!(f.client.response_milliseconds(&offering), Err(Error::Limit));
    offering.direction = crate::modules::board::offering::Direction::Wanted;
    assert!(matches!(
        f.client.mode(&offering, &challenge()),
        Err(Error::Denied)
    ));
    let mut empty = expectations();
    set(&mut empty, "expectedByte", Value::Bytes(vec![]));
    assert_eq!(
        f.client.outcome(
            &challenge(),
            Some(&evidence(b'b')),
            Some(&empty),
            Timing::Within(1)
        ),
        Check::Inconclusive
    );
    f.local
        .leases
        .lock()
        .unwrap()
        .get_mut(&(inv.key, inv.provider, inv.requester, [8; 32]))
        .unwrap()
        .state = State::Removed;
    assert_eq!(
        f.client.challenge(&inv, &challenge()),
        Err(Error::Unavailable)
    );
}
