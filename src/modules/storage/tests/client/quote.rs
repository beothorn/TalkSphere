//! Original provider-scoped lease expectations guard Storage quote dimensions, renewal factors and input ownership.
use super::*;
use crate::modules::services::{
    authority::{Action, Authority},
    contract::{Context, Limits as ServiceLimits},
    pricing::Price,
    registry::{Registry, Standards},
};
use std::time::{Duration, Instant};
// Store data stays out of metadata and all four ordinary operations calculate exact accepted prices through installed requester ports.
#[test]
fn all_operation_prices_and_provided_wanted_store() {
    let f = fixture();
    for wanted in [false, true] {
        let (inv, input, _) = store_fixture(wanted);
        let metadata = f
            .client
            .assessment_metadata_with_peer(peer(&inv, false), &inv.offering, &input, B)
            .unwrap();
        let Value::Object(fields) = &metadata else {
            panic!("metadata object")
        };
        assert_eq!(fields.len(), 3);
        assert!(
            !fields
                .iter()
                .any(|(name, _)| matches!(name.as_str(), "data" | "contentHash"))
        );
        f.client.validate_input(&metadata, &input, B).unwrap();
        f.client
            .validate_assessment_with_peer(
                peer(&inv, false),
                &inv.offering,
                &input,
                &metadata,
                &inv.factors,
                B,
            )
            .unwrap();
        assert_eq!(
            f.client
                .pricing_with_peer(peer(&inv, false), &inv.offering, &metadata, &inv.factors),
            Ok(Price::Fixed(17))
        );
        assert!(
            f.local
                .leases
                .lock()
                .unwrap()
                .keys()
                .all(|(key, _, _, _)| key.stream != 1)
        );
    }
    let (renew, input, _, _) = renew_fixture();
    for provider_side in [false, true] {
        assert_eq!(
            f.client.pricing_with_peer(
                peer(&renew, provider_side),
                &renew.offering,
                &input,
                &renew.factors
            ),
            Ok(Price::Fixed(15))
        );
    }
    let (inv, input, _, _) = retrieve();
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&inv, false), &inv.offering, &input, &inv.factors),
        Ok(Price::Fixed(2))
    );
    let (inv, input, _, _) = remove_fixture();
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&inv, false), &inv.offering, &input, &inv.factors),
        Ok(Price::Fixed(2))
    );
}
// Identical opaque lease IDs at two providers have independent sizes and immutable original attempt snapshots, even when the issuer is shared.
#[test]
fn provider_collision_and_original_attempt_are_not_issuer_lookups() {
    let f = fixture();
    let (inv, input, _, lease) = renew_fixture();
    let other_provider = IdentityId([9; 32]);
    let mut other = inv.clone();
    other.provider = other_provider;
    other.publisher = other_provider;
    other.key.connection = [7; 32];
    let mut other_lease = lease.clone();
    other_lease.provider = other_provider;
    other_lease.size_bytes = 5;
    f.local.leases.lock().unwrap().insert(
        (other.key, other_provider, other.requester, lease.id),
        other_lease,
    );
    let factors = Value::Object(vec![("sizeBytes".into(), Value::Uint64(5))]);
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&other, false), &other.offering, &input, &factors),
        Ok(Price::Fixed(25))
    );
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&inv, false), &inv.offering, &input, &factors),
        Err(Error::Invalid)
    );
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&other, false), &other.offering, &input, &inv.factors),
        Err(Error::Invalid)
    );
    let mut next = inv.clone();
    next.key.stream = 8;
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&next, false), &next.offering, &input, &next.factors),
        Err(Error::Denied)
    );
    assert_eq!(
        f.client.pricing_for(&inv.offering, &input, &inv.factors),
        Err(Error::Unsupported)
    );
}
// Changed local bodies, forged dimensions, factors and known terminal lease state fail before accepting a quote or input.
#[test]
fn forged_projection_factors_and_bounds() {
    let f = fixture();
    let (inv, input, _) = store_fixture(false);
    let metadata = f.client.assessment_metadata(&input, B).unwrap();
    let mut changed = metadata.clone();
    set(&mut changed, "sizeBytes", Value::Uint64(4));
    assert_eq!(
        f.client.validate_input(&changed, &input, B),
        Err(Error::Invalid)
    );
    assert_eq!(
        f.client.validate_assessment_with_peer(
            peer(&inv, false),
            &inv.offering,
            &input,
            &changed,
            &Value::Object(vec![]),
            B
        ),
        Err(Error::Invalid)
    );
    let mut short = input.clone();
    set(&mut short, "data", Value::Bytes(b"ab".to_vec()));
    assert_eq!(f.client.assessment_metadata(&short, B), Err(Error::Invalid));
    assert!(matches!(
        f.client
            .assessment_metadata(&input, Budget { bytes: 1, ..B }),
        Err(Error::Limit)
    ));
    let (inv, input, _, lease) = renew_fixture();
    let mut stale = input.clone();
    set(&mut stale, "expectedExpiresAt", Value::Uint64(1009));
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&inv, false), &inv.offering, &stale, &inv.factors),
        Err(Error::Conflict)
    );
    f.local
        .leases
        .lock()
        .unwrap()
        .get_mut(&(inv.key, inv.provider, inv.requester, lease.id))
        .unwrap()
        .state = State::Removed;
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&inv, false), &inv.offering, &input, &inv.factors),
        Err(Error::Unavailable)
    );
}
// Public standard registration installs genuine requester code while retaining explicit provider absence.
#[test]
fn real_requester_registration_does_not_publish_a_provider() {
    let f = fixture();
    let (inv, _, _) = store_fixture(false);
    let catalogue = Arc::new(Catalogue::new(BoardLimits::default()).unwrap());
    assert_eq!(catalogue.validate(f.client.definition()), Ok(()));
    let authority = Arc::new(Authority::new());
    let grant = authority
        .grant(inv.requester, Action::Register, 65536)
        .unwrap();
    let registry = Registry::new(
        inv.requester,
        authority,
        ServiceLimits::default(),
        catalogue,
    )
    .unwrap();
    let context = Context {
        version: 1,
        grant: &grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    registry
        .register(&context, Arc::new(f.client), None)
        .unwrap();
    let registered = registry.select("cr2se.storage", 1, false).unwrap();
    assert!(registered.requester().checks().is_some());
    assert!(matches!(
        registry.select("cr2se.storage", 1, true),
        Err(Error::Unsupported)
    ));
}

// Original-key snapshots survive later catalog entries; a stale new attempt cannot borrow another stream's old guard.
#[test]
fn later_snapshot_does_not_replace_original_quote_context() {
    let f = fixture();
    let (inv, input, output, lease) = renew_fixture();
    let mut next = inv.clone();
    next.key.stream = 8;
    let mut later = lease.clone();
    later.expires_at = 1020;
    f.local
        .leases
        .lock()
        .unwrap()
        .insert((next.key, next.provider, next.requester, lease.id), later);
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&next, false), &next.offering, &input, &next.factors),
        Err(Error::Conflict)
    );
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&inv, false), &inv.offering, &input, &inv.factors),
        Ok(Price::Fixed(15))
    );
    f.client.validate_result(&inv, &input, &output).unwrap();
}
// Peer scope, known active ownership, empty operation factors and subtraction-safe range limits all fail closed before pricing.
#[test]
fn invalid_association_and_current_range_limits() {
    let f = fixture();
    let (inv, input, _, lease) = retrieve();
    let forged = Peer {
        local: IdentityId([9; 32]),
        remote: IdentityId([8; 32]),
        key: inv.key,
    };
    assert_eq!(
        f.client
            .pricing_with_peer(forged, &inv.offering, &input, &inv.factors),
        Err(Error::Denied)
    );
    assert_eq!(
        f.client
            .validate_assessment(&input, &input, &inv.factors, B),
        Err(Error::Unsupported)
    );
    let extra = Value::Object(vec![("sizeBytes".into(), Value::Uint64(3))]);
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&inv, false), &inv.offering, &input, &extra),
        Err(Error::Invalid)
    );
    for (name, value) in [("offset", u64::MAX), ("length", u64::MAX), ("length", 0)] {
        let mut bad = input.clone();
        set(&mut bad, name, Value::Uint64(value));
        assert!(
            f.client
                .pricing_with_peer(peer(&inv, false), &inv.offering, &bad, &inv.factors)
                .is_err()
        );
    }
    let mut offering = inv.offering.clone();
    offering.source["info"]["maximumRetrieveBytes"] = serde_json::json!(1);
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&inv, false), &offering, &input, &inv.factors),
        Err(Error::Limit)
    );
    offering.direction = crate::modules::board::offering::Direction::Wanted;
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&inv, false), &offering, &input, &inv.factors),
        Err(Error::Denied)
    );
    f.local
        .leases
        .lock()
        .unwrap()
        .get_mut(&(inv.key, inv.provider, inv.requester, lease.id))
        .unwrap()
        .provider = IdentityId([9; 32]);
    assert_eq!(
        f.client
            .pricing_with_peer(peer(&inv, false), &inv.offering, &input, &inv.factors),
        Err(Error::Denied)
    );
}
