//! PP-07 wire projections use literal family field names and exact decimal oracles; these tests install no standard provider.
use super::*;
// All five specified provider-dependent factors preserve exact uint64 strings and reject invented, missing or mistyped fields.
#[test]
fn specified_pricing_inputs_have_exact_independent_wire_projections() {
    for (service, operation, field) in [
        ("cr2se.public-file-sharing", "manifest", "manifestBytes"),
        ("cr2se.public-file-sharing", "download", "downloadBytes"),
        ("cr2se.storage", "renew", "sizeBytes"),
        ("cr2se.messaging", "recover", "payloadSize"),
        ("cr2se.messaging", "renew", "payloadSize"),
    ] {
        for (number, decimal) in [
            (0, "0"),
            (9_007_199_254_740_993, "9007199254740993"),
            (u64::MAX, "18446744073709551615"),
        ] {
            let fields = Value::Object(vec![(field.into(), Value::Uint64(number))]);
            let expected = serde_json::json!({field:decimal});
            assert_eq!(
                quote::factors(service, Some(operation), &fields),
                Ok((expected.clone(), None))
            );
            let wire = serde_json::json!({"pricingInputs":expected});
            assert_eq!(factors(service, Some(operation), &wire).unwrap(), fields);
            for bad in [
                serde_json::json!({}),
                serde_json::json!({"invented":decimal}),
                serde_json::json!({field:number}),
                serde_json::json!({field:decimal,"extra":"1"}),
            ] {
                assert!(
                    factors(
                        service,
                        Some(operation),
                        &serde_json::json!({"pricingInputs":bad})
                    )
                    .is_err()
                );
            }
        }
    }
}
// Removal release credits have their own exact field and cannot enter arbitrary operation pricing inputs.
#[test]
fn removal_release_and_unknown_factors_cannot_change_price_authority() {
    let value = Value::Object(vec![("releaseCredits".into(), Value::Uint64(u64::MAX))]);
    assert_eq!(
        quote::factors("cr2se.messaging", Some("remove"), &value),
        Ok((serde_json::json!({}), Some(u64::MAX)))
    );
    let wire = serde_json::json!({"pricingInputs":{},"releaseCredits":"18446744073709551615"});
    assert_eq!(
        factors("cr2se.messaging", Some("remove"), &wire).unwrap(),
        value
    );
    assert!(factors("cr2se.messaging", Some("recover"), &wire).is_err());
    assert!(
        factors(
            "cr2se.messaging",
            Some("remove"),
            &serde_json::json!({"pricingInputs":{"payloadSize":"1"},"releaseCredits":"1"})
        )
        .is_err()
    );
    assert!(quote::factors("org.example.custom", None, &value).is_err());
    assert!(
        quote::factors(
            "cr2se.storage",
            Some("renew"),
            &Value::String("not fields".into())
        )
        .is_err()
    );
    assert!(quote::factors("cr2se.messaging", Some("remove"), &Value::Object(vec![])).is_err());
}
