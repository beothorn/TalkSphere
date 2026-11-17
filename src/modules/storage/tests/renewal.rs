//! Renewal quote guards compare trusted owner state with independent requester size expectations.
use super::*;
use crate::modules::{
    services::{contract::OperationKey, handler::Invocation},
    storage::{
        contract::Lease,
        requester::store::{self, Policy},
        retention::Anchor,
        test_support::contracts::store as store_fixture,
    },
};
const B: Budget = Budget {
    depth: 8,
    nodes: 128,
    bytes: 4096,
};

// Retain independently specified abc metadata and use original authenticated role identities for a new renewal quote.
fn fixture() -> (Invocation, Peer, Image, Value) {
    let (inv, input, output) = store_fixture(false);
    let lease = store::validate(
        &inv,
        &input,
        &output,
        B,
        Policy {
            observed_unix_seconds: 1000,
            maximum_clock_difference_seconds: 0,
        },
    )
    .unwrap();
    let peer = Peer {
        local: inv.provider,
        remote: inv.requester,
        key: OperationKey {
            stream: 2,
            ..inv.key
        },
    };
    let image = Image {
        provider: inv.provider,
        anchor: Anchor::new(time(0)),
        leases: vec![lease],
        operations: vec![],
    };
    let input = Value::Object(vec![
        ("operation".into(), Value::String("renew".into())),
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("expectedExpiresAt".into(), Value::Uint64(1010)),
        ("additionalSeconds".into(), Value::Uint64(10)),
    ]);
    (inv, peer, image, input)
}
// Observe independent monotonic progress during a stable provider boot.
fn time(elapsed: u64) -> Time {
    Time {
        unix_seconds: 1000 + elapsed,
        elapsed_seconds: elapsed,
        boot: [1; 16],
    }
}
// Exercise one assessed input with a trusted unmodified clock, without staging or extending any lease.
fn assess_fixture(
    peer: &Peer,
    offering: &Offering,
    image: &Image,
    input: Value,
    maximum: u64,
) -> Result<Renewal, Error> {
    assess(
        Context {
            peer,
            offering,
            image,
            now: time(1),
            reboot_elapsed: None,
        },
        &Input::Complete(input),
        B,
        maximum,
    )
}
// Replace one exact logical dimension to make rejection causes independent.
fn set(value: &mut Value, name: &str, replacement: Value) {
    let Value::Object(fields) = value else {
        panic!("fixture object")
    };
    fields.iter_mut().find(|(n, _)| n == name).unwrap().1 = replacement;
}
// The provider quotes retained size 3 and ten seconds at 1/2 credit per byte-second, independently checked by the requester.
#[test]
fn exact_quote_and_unchanged_lease() {
    let (inv, peer, image, input) = fixture();
    let result = assess_fixture(&peer, &inv.offering, &image, input.clone(), 15).unwrap();
    assert_eq!(result.price, 15);
    assert_eq!(result.expires_at, 1020);
    assert_eq!(result.assessment.metadata, input);
    assert_eq!(
        result.assessment.factors,
        Value::Object(vec![("sizeBytes".into(), Value::Uint64(3))])
    );
    assert_eq!(
        price(
            &inv.offering,
            &input,
            &result.assessment.factors,
            &image.leases[0],
            B
        ),
        Ok(Price::Fixed(15))
    );
    assert!(result.assessment.pin.is_none());
    assert_eq!(image.leases[0].expires_at, 1010);
    assert_eq!(image.leases[0].state, State::Active);
    assert!(matches!(
        assess_fixture(&peer, &inv.offering, &image, input, 14),
        Err(Error::Insufficient)
    ));
}
// Missing/foreign leases share Denied; expiry and stale guards are checked only after ownership authorization.
#[test]
fn owner_clock_and_guard_failures() {
    let (inv, peer, image, input) = fixture();
    for index in 0..6 {
        let mut bad = image.clone();
        match index {
            0 => bad.leases.clear(),
            1 => bad.leases[0].requester.0 = [9; 32],
            2 => bad.leases[0].provider.0 = [9; 32],
            3 => bad.leases[0].state = State::Removed,
            4 => bad.leases[0].expires_at = 1001,
            _ => bad.leases[0].expires_at = 1011,
        }
        let expected = if index < 3 {
            Error::Denied
        } else if index < 5 {
            Error::Unavailable
        } else {
            Error::Conflict
        };
        assert!(
            matches!(assess_fixture(&peer,&inv.offering,&bad,input.clone(),100),Err(e) if e==expected)
        );
    }
    let mut foreign = peer;
    foreign.remote.0 = [9; 32];
    foreign.key.requester = foreign.remote;
    assert!(matches!(
        assess_fixture(&foreign, &inv.offering, &image, input.clone(), 100),
        Err(Error::Denied)
    ));
    assert!(matches!(
        assess(
            Context {
                peer: &peer,
                offering: &inv.offering,
                image: &image,
                now: Time {
                    boot: [2; 16],
                    ..time(1)
                },
                reboot_elapsed: None
            },
            &Input::Complete(input),
            B,
            100
        ),
        Err(Error::Unavailable)
    ));
}
// Every duration and addition remains checked, and rejected terms never change the original retention obligation.
#[test]
fn limits_overflows_and_unresolved_input() {
    let (inv, peer, image, input) = fixture();
    for seconds in [0, 101, u64::MAX] {
        let mut bad = input.clone();
        set(&mut bad, "additionalSeconds", Value::Uint64(seconds));
        assert!(assess_fixture(&peer, &inv.offering, &image, bad, 1000).is_err());
    }
    let mut large = image.clone();
    large.leases[0].accepted_at = u64::MAX - 100;
    large.leases[0].expires_at = u64::MAX - 1;
    let mut bad = input.clone();
    set(&mut bad, "expectedExpiresAt", Value::Uint64(u64::MAX - 1));
    assert!(matches!(
        assess_fixture(&peer, &inv.offering, &large, bad, 100),
        Err(Error::Limit)
    ));
    let mut total = input.clone();
    set(&mut total, "additionalSeconds", Value::Uint64(100));
    let mut old = image.clone();
    old.leases[0].expires_at = 1190;
    set(&mut total, "expectedExpiresAt", Value::Uint64(1190));
    assert!(matches!(
        assess_fixture(&peer, &inv.offering, &old, total, 1000),
        Err(Error::Limit)
    ));
    assert!(matches!(
        assess(
            Context {
                peer: &peer,
                offering: &inv.offering,
                image: &image,
                now: time(1),
                reboot_elapsed: None
            },
            &Input::Body {
                length: 32,
                text: false
            },
            B,
            100
        ),
        Err(Error::Unsupported)
    ));
    assert_eq!(image.leases[0].expires_at, 1010);
}
// Sole exact uint64 size factors are checked against independently retained metadata, including sizes beyond current store limits.
#[test]
fn exact_factor_shapes_and_uint64_size() {
    let (mut inv, _, image, input) = fixture();
    let lease = &image.leases[0];
    for factors in [
        Value::Object(vec![]),
        Value::Bool(true),
        Value::Object(vec![("sizeBytes".into(), Value::Uint64(4))]),
        Value::Object(vec![("sizeBytes".into(), Value::String("3".into()))]),
        Value::Object(vec![("wrongName".into(), Value::Uint64(3))]),
        Value::Object(vec![
            ("sizeBytes".into(), Value::Uint64(3)),
            ("extra".into(), Value::Uint64(3)),
        ]),
    ] {
        assert!(price(&inv.offering, &input, &factors, lease, B).is_err());
    }
    let mut huge: Lease = lease.clone();
    huge.size_bytes = u64::MAX;
    let factors = Value::Object(vec![("sizeBytes".into(), Value::Uint64(u64::MAX))]);
    assert_eq!(
        price(&inv.offering, &input, &factors, &huge, B),
        Err(Error::Limit)
    );
    // A zero numerator makes the exact final price fit without allocating or truncating a uint64-sized object.
    inv.offering.source["pricing"]["renewalByteStorageDurationFactor"]["numerator"] =
        serde_json::json!(0);
    assert_eq!(
        price(&inv.offering, &input, &factors, &huge, B),
        Ok(Price::Fixed(1))
    );
}
