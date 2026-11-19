//! Paid check proposals use actual Storage rules and requester admission while private expected bytes remain outside wire input.
use super::*;
// Real check requester configuration uses independent Ledger read authority and actual installed Storage Rules, without a provider handler.
fn owner(endpoint: &Endpoint) -> wire::checks::Owner {
    let read = endpoint
        .ledger_authority
        .grant(
            endpoint.association.local,
            ledger::authority::Action::Read,
            16_777_216,
        )
        .unwrap();
    let gate = Arc::new(
        wire::admission::LedgerGate::new(
            endpoint.association.local,
            endpoint.ledger.clone(),
            read,
            Default::default(),
        )
        .unwrap(),
    );
    wire::checks::Owner::new(wire::checks::Config {
        local: endpoint.association.local,
        authority: endpoint.authority.clone(),
        services: endpoint.services.clone(),
        registry: endpoint.registry.clone(),
        limits: WireLimits::default(),
        gate,
        entropy: Arc::new(wire::quote::SystemEntropy),
    })
    .unwrap()
}
// The original check is a separate positive-price operation; no quote/proposal serializes private expected bytes or consumes included store allowance.
#[test]
fn challenge_only_paid_check_requires_explicit_consent_and_reservation() {
    let f = fixture();
    let (inv, _, _, _) = renew_fixture();
    let endpoint = endpoint(f.client, &inv);
    let owner = owner(&endpoint);
    let request = endpoint
        .authority
        .grant(inv.requester, wire::authority::Action::Request, 16_777_216)
        .unwrap();
    let invoke = endpoint
        .authority
        .grant(inv.requester, wire::authority::Action::Invoke, 16_777_216)
        .unwrap();
    let services = endpoint
        .services
        .grant(
            inv.requester,
            services::authority::Action::Request,
            16_777_216,
        )
        .unwrap();
    let coordinate = endpoint
        .services
        .grant(
            inv.requester,
            services::authority::Action::Coordinate,
            16_777_216,
        )
        .unwrap();
    let challenge = Value::Object(vec![
        ("leaseId".into(), Value::Bytes(vec![8; 32])),
        ("offset".into(), Value::Uint64(1)),
    ]);
    let original = owner
        .request(
            &pc(&request),
            &sc(&services),
            endpoint.association,
            endpoint.selection.clone(),
            challenge.clone(),
            Spending {
                maximum: 3,
                payment: Payment::default(),
            },
        )
        .unwrap();
    let proposed = Message::parse(original.bytes(), 65536).unwrap();
    assert_eq!(proposed.value()["mode"], "check");
    assert_eq!(proposed.value()["input"].as_object().unwrap().len(), 2);
    assert!(proposed.value()["input"].get("expectedByte").is_none());
    let quote = endpoint.quote(3, json!({}));
    let candidate = owner
        .validate(
            &pc(&invoke),
            &sc(&services),
            original,
            endpoint.association,
            &quote,
        )
        .unwrap();
    assert_eq!(candidate.price(), 3);
    assert_eq!(endpoint.available(), CREDIT);
    assert_eq!(endpoint.phase(), None);
    let hash = candidate.hash();
    let consent = owner.consent(&pc(&request), candidate, hash, 3).unwrap();
    let current = Captured(endpoint.selection.clone());
    let outgoing = owner
        .reserve(
            &pc(&request),
            Admission {
                quote_context: &sc(&services),
                coordinate_context: &sc(&coordinate),
                current: &current,
                runtime: &endpoint.runtime,
            },
            consent,
        )
        .unwrap();
    assert_eq!(outgoing.invocation().price, 3);
    assert_eq!(endpoint.available(), CREDIT - 3);
    assert_eq!(endpoint.phase(), Some(services::runtime::Phase::Executing));
    let mut private = challenge;
    if let Value::Object(fields) = &mut private {
        fields.push(("expectedByte".into(), Value::Bytes(vec![b'b'])));
    }
    assert!(
        owner
            .request(
                &pc(&request),
                &sc(&services),
                endpoint.association,
                endpoint.selection.clone(),
                private,
                Spending {
                    maximum: 3,
                    payment: Payment::default()
                }
            )
            .is_err()
    );
    assert_eq!(endpoint.available(), CREDIT - 3);
}
