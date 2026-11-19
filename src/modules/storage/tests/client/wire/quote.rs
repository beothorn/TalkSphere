//! Actual Storage requester codecs and real Board selection validate remote quote economics before original-input consent.
use super::*;
// All four ordinary operations use real requester ports and exact wire factors without reserving credits during proposal/validation.
#[test]
fn all_ordinary_quotes_are_verified_before_consent() {
    let (store, input, _) = store_fixture(false);
    let (renew, renew_input, _, _) = renew_fixture();
    let (read, read_input, _, _) = retrieve();
    let (remove, remove_input, _, _) = remove_fixture();
    for (inv, input, price, factors) in [
        (store, input, 17, json!({})),
        (renew, renew_input, 15, json!({"sizeBytes":"3"})),
        (read, read_input, 2, json!({})),
        (remove, remove_input, 2, json!({})),
    ] {
        let f = fixture();
        let endpoint = endpoint(f.client, &inv);
        assert!(endpoint.family.cached_retry(&input).unwrap().is_none());
        let original = endpoint.request(input, CREDIT);
        let proposed = Message::parse(original.bytes(), 65536).unwrap();
        assert_eq!(proposed.value()["mode"], "invoke");
        assert_eq!(proposed.value()["service"], "cr2se.storage");
        let candidate = endpoint
            .candidate(original, &endpoint.quote(price, factors))
            .unwrap();
        assert_eq!(candidate.price(), services::pricing::Price::Fixed(price));
        assert_eq!(endpoint.available(), CREDIT);
        assert_eq!(endpoint.phase(), None);
    }
}
// Forged retained size, fixed price, issuer and association cannot become a quote candidate or consume funds.
#[test]
fn forged_quotes_and_changed_terms_require_exact_consent() {
    for case in 0..5 {
        let f = fixture();
        let (inv, input, _, _) = renew_fixture();
        let endpoint = endpoint(f.client, &inv);
        let request = endpoint.request(input, CREDIT);
        let mut value = endpoint.quote(15, json!({"sizeBytes":"3"})).value().clone();
        match case {
            0 => value["pricingInputs"]["sizeBytes"] = "4".into(),
            1 => value["maximumPrice"] = "14".into(),
            2 => value["pricingInputs"]["extra"] = "1".into(),
            3 => {
                value["creditIssuer"] =
                    crate::modules::identity::text::format(IdentityId([9; 32])).into()
            }
            _ => value["offering"]["description"] = "Changed contract".into(),
        }
        let message = Message::parse(&serde_json::to_vec(&value).unwrap(), 65536).unwrap();
        assert!(endpoint.candidate(request, &message).is_err());
        assert_eq!(endpoint.available(), CREDIT);
        assert_eq!(endpoint.phase(), None);
    }
    let f = fixture();
    let (inv, input, _) = store_fixture(false);
    let endpoint = endpoint(f.client, &inv);
    let mut value = endpoint.quote(21, json!({})).value().clone();
    value["offering"]["pricing"]["byteUsageFactor"]["numerator"] = json!(4);
    let message = Message::parse(&serde_json::to_vec(&value).unwrap(), 65536).unwrap();
    let candidate = endpoint
        .candidate(endpoint.request(input, CREDIT), &message)
        .unwrap();
    assert_eq!(candidate.price(), services::pricing::Price::Fixed(21));
    let grant = endpoint
        .authority
        .grant(inv.requester, wire::authority::Action::Request, 16_777_216)
        .unwrap();
    assert!(matches!(
        endpoint
            .client
            .consent(&pc(&grant), candidate, [0; 32], CREDIT),
        Err(wire::Error::Denied)
    ));
    assert_eq!(endpoint.available(), CREDIT);
    assert_eq!(endpoint.phase(), None);
}
