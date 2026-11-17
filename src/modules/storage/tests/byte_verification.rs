//! Private expected bytes and original transport stamps produce conservative independent byte-check outcomes.
use super::*;
use crate::modules::{
    services::{checks::Invocation, handler::Blob},
    storage::test_support::contracts::store as store_fixture,
};
const B: Budget = Budget {
    depth: 8,
    nodes: 128,
    bytes: 4096,
};
// Independently specify accepted check economics and exact abc byte evidence; no provider helper constructs expected results.
fn fixture() -> (Invocation, Value, Value, Expected) {
    let (inv, _, _) = store_fixture(false);
    let invocation = Invocation {
        key: inv.key,
        requester: inv.requester,
        provider: inv.provider,
        issuer: inv.issuer,
        offering: inv.offering,
        definition: inv.definition,
        quote: Blob::new(b"accepted paid check".to_vec(), 32).unwrap(),
        price: 3,
        response_milliseconds: 5000,
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(60),
    };
    let challenge = Value::Object(vec![
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("offset".into(), Value::Uint64(1)),
    ]);
    let evidence = Value::Object(vec![
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("offset".into(), Value::Uint64(1)),
        ("value".into(), Value::Bytes(vec![b'b'])),
    ]);
    (
        invocation,
        challenge,
        evidence,
        Expected {
            lease_id: [8; 32],
            offset: 1,
            byte: b'b',
        },
    )
}
// Preserve original local clock observations even when verification runs long after the network callback.
fn stamp(milliseconds: u64) -> Stamp {
    Stamp {
        epoch: [1; 32],
        milliseconds,
    }
}
// Model complete challenge send, complete evidence and normal response END on one trusted original local epoch.
fn observations(end: u64) -> Observations {
    Observations {
        sent: Some(stamp(1000)),
        received: Some(stamp(end)),
        ended: Some(stamp(end)),
    }
}
// Replace one peer-controlled logical field without touching private expectations or transport observations.
fn set(value: &mut Value, name: &str, replacement: Value) {
    let Value::Object(fields) = value else {
        panic!("fixture object")
    };
    fields.iter_mut().find(|(n, _)| n == name).unwrap().1 = replacement;
}
// Exact timeout equality passes; the next millisecond is inconclusive, and only complete timely incorrect bytes fail.
#[test]
fn exact_timeout_private_comparison_and_delayed_verification() {
    let (mut inv, input, evidence, expected) = fixture();
    // A stale worker deadline cannot replace original accepted transport timing in this pure verifier.
    inv.deadline = std::time::Instant::now() - std::time::Duration::from_secs(1);
    for end in [1000, 5999, 6000] {
        assert_eq!(
            verify(
                &inv,
                &input,
                Reply::Evidence {
                    peer: inv.provider,
                    value: &evidence
                },
                Some(&expected),
                Activity::Active,
                observations(end),
                B
            ),
            Check::Pass
        );
    }
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Evidence {
                peer: inv.provider,
                value: &evidence
            },
            Some(&expected),
            Activity::Active,
            observations(6001),
            B
        ),
        Check::Inconclusive
    );
    let mut wrong = evidence.clone();
    set(&mut wrong, "value", Value::Bytes(vec![b'c']));
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Evidence {
                peer: inv.provider,
                value: &wrong
            },
            Some(&expected),
            Activity::Active,
            observations(2000),
            B
        ),
        Check::Fail
    );
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Evidence {
                peer: inv.provider,
                value: &wrong
            },
            None,
            Activity::Active,
            observations(2000),
            B
        ),
        Check::Inconclusive
    );
}
// Missing/foreign/ill-formed responses, mismatch associations and provider-computed outcomes establish no trustworthy conclusion.
#[test]
fn malformed_foreign_missing_and_private_binding() {
    let (inv, input, evidence, expected) = fixture();
    let Value::Object(fields) = &evidence else {
        unreachable!()
    };
    for i in 0..fields.len() {
        let mut missing = fields.clone();
        missing.remove(i);
        assert_eq!(
            verify(
                &inv,
                &input,
                Reply::Evidence {
                    peer: inv.provider,
                    value: &Value::Object(missing)
                },
                Some(&expected),
                Activity::Active,
                observations(2000),
                B
            ),
            Check::Inconclusive
        );
    }
    for (name, value) in [
        ("leaseId", Value::Bytes(vec![9; 32])),
        ("offset", Value::Uint64(2)),
        ("value", Value::Bytes(vec![])),
        ("value", Value::Bytes(vec![b'b'; 2])),
    ] {
        let mut bad = evidence.clone();
        set(&mut bad, name, value);
        assert_eq!(
            verify(
                &inv,
                &input,
                Reply::Evidence {
                    peer: inv.provider,
                    value: &bad
                },
                Some(&expected),
                Activity::Active,
                observations(2000),
                B
            ),
            Check::Inconclusive
        );
    }
    let mut claim = evidence.clone();
    if let Value::Object(fields) = &mut claim {
        fields.push(("outcome".into(), Value::String("pass".into())));
    }
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Evidence {
                peer: inv.provider,
                value: &claim
            },
            Some(&expected),
            Activity::Active,
            observations(2000),
            B
        ),
        Check::Inconclusive
    );
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Evidence {
                peer: inv.requester,
                value: &evidence
            },
            Some(&expected),
            Activity::Active,
            observations(2000),
            B
        ),
        Check::Inconclusive
    );
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Missing,
            Some(&expected),
            Activity::Active,
            observations(2000),
            B
        ),
        Check::Inconclusive
    );
    let wrong = Expected {
        offset: 2,
        ..expected
    };
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Evidence {
                peer: inv.provider,
                value: &evidence
            },
            Some(&wrong),
            Activity::Active,
            observations(2000),
            B
        ),
        Check::Inconclusive
    );
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Evidence {
                peer: inv.provider,
                value: &evidence
            },
            Some(&expected),
            Activity::Active,
            observations(2000),
            Budget { bytes: 1, ..B }
        ),
        Check::Inconclusive
    );
}
// Original send/evidence/end ordering and matching epochs are mandatory; queue delay cannot be used to create new timestamps.
#[test]
fn missing_cross_epoch_and_abnormal_timing() {
    let (inv, input, evidence, expected) = fixture();
    for stamps in [
        Observations::default(),
        Observations {
            sent: None,
            ..observations(2000)
        },
        Observations {
            received: None,
            ..observations(2000)
        },
        Observations {
            ended: None,
            ..observations(2000)
        },
        Observations {
            received: Some(stamp(999)),
            ..observations(2000)
        },
        Observations {
            received: Some(stamp(2001)),
            ..observations(2000)
        },
        Observations {
            ended: Some(Stamp {
                epoch: [2; 32],
                milliseconds: 2000,
            }),
            ..observations(2000)
        },
        Observations {
            received: Some(Stamp {
                epoch: [2; 32],
                milliseconds: 2000,
            }),
            ..observations(2000)
        },
    ] {
        assert_eq!(
            verify(
                &inv,
                &input,
                Reply::Evidence {
                    peer: inv.provider,
                    value: &evidence
                },
                Some(&expected),
                Activity::Active,
                stamps,
                B
            ),
            Check::Inconclusive
        );
    }
    for activity in [Activity::Inactive, Activity::Ambiguous] {
        assert_eq!(
            verify(
                &inv,
                &input,
                Reply::Evidence {
                    peer: inv.provider,
                    value: &evidence
                },
                Some(&expected),
                activity,
                observations(2000),
                B
            ),
            Check::Inconclusive
        );
    }
}
// Only an explicit authenticated authorized-active unavailability report can fail without a byte; generic communication loss is inconclusive.
#[test]
fn authenticated_unavailability_and_invalid_contract() {
    let (mut inv, input, _, expected) = fixture();
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Unavailable { peer: inv.provider },
            Some(&expected),
            Activity::Active,
            observations(2000),
            B
        ),
        Check::Fail
    );
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Unavailable {
                peer: inv.requester
            },
            Some(&expected),
            Activity::Active,
            observations(2000),
            B
        ),
        Check::Inconclusive
    );
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Unavailable { peer: inv.provider },
            Some(&expected),
            Activity::Ambiguous,
            observations(2000),
            B
        ),
        Check::Inconclusive
    );
    inv.price = 0;
    assert_eq!(
        verify(
            &inv,
            &input,
            Reply::Unavailable { peer: inv.provider },
            Some(&expected),
            Activity::Active,
            observations(2000),
            B
        ),
        Check::Inconclusive
    );
}
