//! Independent Storage catalogue admission covers complete schema tampering, compatible metadata and bounded offering-specific definitions.
use super::*;
use crate::modules::{
    services::{
        handler::{Blob, Check, Requester},
        validation::Semantics,
    },
    storage::test_support::contracts::store as fixture,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Keep incoming definition mutations independent of catalogue factory output and preserve the public support-marker attack surface.
fn document(value: &Value, support: Support) -> Definition {
    Definition {
        document: Document::parse(&serde_json::to_vec(value).unwrap(), Limits::default()).unwrap(),
        support,
    }
}
// Provided/wanted definitions bind their own labels without changing the pinned operation schemas or authorizing provider code.
#[test]
fn offered_identity_and_both_directions() {
    let catalogue = Catalogue::new(Limits::default()).unwrap();
    for wanted in [false, true] {
        let (mut inv, _, _) = fixture(wanted);
        inv.offering.id = "another-storage-selection".into();
        inv.offering.description = "A current Storage offer.".into();
        let definition = catalogue.definition(&inv.offering).unwrap();
        assert_eq!(
            definition.document.value()["id"],
            "another-storage-selection"
        );
        assert_eq!(
            definition.document.value()["description"],
            "A current Storage offer."
        );
        assert_eq!(
            definition.document.value()["input"]["fields"]["sizeBytes"]["type"],
            "uint64"
        );
        assert_eq!(
            definition.document.value()["check"]["output"]["fields"]["value"]["optional"],
            true
        );
        assert_eq!(catalogue.validate(&definition), Ok(()));
    }
}
// Every required schema/known field and its exact logical type/optionality is checked independently of a forged Known marker.
#[test]
fn complete_contract_tamper_matrix() {
    let catalogue = Catalogue::new(Limits::default()).unwrap();
    let (inv, _, _) = fixture(false);
    let original = inv.definition.document.value();
    for section in ["input", "output", "check"] {
        let mut bad = original.clone();
        bad.as_object_mut().unwrap().remove(section);
        assert!(catalogue.validate(&document(&bad, Support::Known)).is_err());
    }
    for section in ["input", "output"] {
        let fields = original[section]["fields"].as_object().unwrap();
        for name in fields.keys() {
            let mut bad = original.clone();
            bad[section]["fields"].as_object_mut().unwrap().remove(name);
            assert_eq!(
                catalogue.validate(&document(&bad, Support::Known)),
                Err(Error::Invalid)
            );
            let mut bad = original.clone();
            bad[section]["fields"][name]["type"] = if fields[name]["type"] == "bool" {
                "bytes"
            } else {
                "bool"
            }
            .into();
            assert_eq!(
                catalogue.validate(&document(&bad, Support::Known)),
                Err(Error::Invalid)
            );
            let mut bad = original.clone();
            bad[section]["fields"][name]["optional"] =
                Value::Bool(fields[name]["optional"] != true);
            assert_eq!(
                catalogue.validate(&document(&bad, Support::Known)),
                Err(Error::Invalid)
            );
        }
    }
    for section in ["input", "output"] {
        for name in original["check"][section]["fields"]
            .as_object()
            .unwrap()
            .keys()
        {
            let mut bad = original.clone();
            bad["check"][section]["fields"]
                .as_object_mut()
                .unwrap()
                .remove(name);
            assert_eq!(
                catalogue.validate(&document(&bad, Support::Known)),
                Err(Error::Invalid)
            );
            let mut bad = original.clone();
            bad["check"][section]["fields"][name]["type"] = "bool".into();
            assert_eq!(
                catalogue.validate(&document(&bad, Support::Known)),
                Err(Error::Invalid)
            );
        }
    }
    let mut unknown = original.clone();
    unknown["check"]["input"]["fields"]["offset"]["type"] = "org.example.unknown".into();
    assert_eq!(
        catalogue.validate(&document(&unknown, Support::Known)),
        Err(Error::Unsupported)
    );
    assert_eq!(
        catalogue.validate(&document(original, Support::Unknown)),
        Err(Error::Unsupported)
    );
    for name in ["creditIssuer", "price", "pricing", "checkPrice", "info"] {
        let mut bad = original.clone();
        bad[name] = Value::Bool(true);
        assert_eq!(
            catalogue.validate(&document(&bad, Support::Known)),
            Err(Error::Invalid)
        );
    }
}
// Human prose, annotations, equivalent numeric spelling and known optional extension fields stay inert; unknown mandatory extensions do not become understood.
#[test]
fn equivalent_descriptions_and_extension_rules() {
    let catalogue = Catalogue::new(Limits::default()).unwrap();
    let (inv, _, _) = fixture(false);
    let mut value = inv.definition.document.value().clone();
    value["id"] = "another-id".into();
    value["description"] = "Another description.".into();
    value["serviceVersion"] = serde_json::from_str("1.0").unwrap();
    value["input"]["fields"]["operation"]["description"] = "Select one standard operation.".into();
    value["input"]["fields"]["operation"]["optional"] = false.into();
    value["input"]["annotation"] = "Inert metadata".into();
    value["input"]["fields"]["extra"] =
        serde_json::json!({"type":"bytes","description":"Optional inert field.","optional":true});
    assert_eq!(
        catalogue.validate(&document(&value, Support::Known)),
        Ok(())
    );
    value["input"]["fields"]["extra"]["optional"] = false.into();
    assert_eq!(
        catalogue.validate(&document(&value, Support::Known)),
        Err(Error::Unsupported)
    );
    value["serviceVersion"] = 2.into();
    assert_eq!(
        catalogue.validate(&document(&value, Support::Known)),
        Err(Error::Unsupported)
    );
}
// Independent limits reject oversized incoming documents and JSON escape expansion before returning a generated contract.
#[test]
fn budgets_and_unsupported_family() {
    assert!(matches!(
        Catalogue::new(Limits {
            nodes: 0,
            ..Limits::default()
        }),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        Catalogue::new(Limits {
            document_bytes: 16,
            string_bytes: 16,
            ..Limits::default()
        }),
        Err(Error::Limit)
    ));
    let catalogue = Catalogue::new(Limits {
        document_bytes: 8192,
        string_bytes: 8192,
        ..Limits::default()
    })
    .unwrap();
    let (mut inv, _, _) = fixture(false);
    inv.offering.description = "\u{0001}".repeat(2000);
    assert!(matches!(
        catalogue.definition(&inv.offering),
        Err(Error::Limit)
    ));
    inv.offering.description = "x".repeat(8193);
    assert!(matches!(
        catalogue.definition(&inv.offering),
        Err(Error::Limit)
    ));
    inv.offering.version = 2;
    assert!(matches!(
        catalogue.definition(&inv.offering),
        Err(Error::Unsupported)
    ));
    let (inv, _, _) = fixture(false);
    let mut large = inv.definition.document.value().clone();
    large["annotation"] = Value::String("x".repeat(8192));
    assert_eq!(
        catalogue.validate(&document(&large, Support::Known)),
        Err(Error::Limit)
    );
}
struct DescriptionOnly(Definition);
impl Semantics for DescriptionOnly {
    // This test-only consumer exposes no executable Storage operation; catalogue recognition is not an implementation.
    fn validate(&self, _: &Value, _: &crate::modules::services::value::Value) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    // Standard Storage values do not permit nonfinite floats.
    fn nonfinite(&self) -> bool {
        false
    }
}
impl Requester for DescriptionOnly {
    // Return only the independently admitted definition for registry admission tests.
    fn definition(&self) -> &Definition {
        &self.0
    }
    // No economic or invocation capability is claimed by this test-only description holder.
    fn terms(&self, _: &Offering) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    // No private expected data or paid check implementation exists in this test-only holder.
    fn verify(
        &self,
        _: &crate::modules::services::value::Value,
        _: &Blob,
        _: &crate::modules::services::value::Value,
        _: Option<&crate::modules::services::value::Value>,
    ) -> Result<Check, Error> {
        Err(Error::Unsupported)
    }
}
// Real Services admission recognizes only the independent standard; neither the catalogue nor a requester-only entry enables provider selection.
#[test]
fn real_registry_keeps_provider_unavailable() {
    use crate::modules::services::{
        authority::{Action, Authority},
        contract::{Context, Limits as ServicesLimits},
        registry::Registry,
    };
    let (inv, _, _) = fixture(false);
    let authority = Arc::new(Authority::new());
    let grant = authority
        .grant(inv.requester, Action::Register, 65536)
        .unwrap();
    let context = Context {
        version: 1,
        grant: &grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let registry = Registry::new(
        inv.requester,
        authority,
        ServicesLimits::default(),
        Arc::new(Catalogue::new(Limits::default()).unwrap()),
    )
    .unwrap();
    assert!(registry.select("cr2se.storage", 1, false).is_err());
    let mut changed = inv.definition.document.value().clone();
    changed["input"]["fields"]["length"]["type"] = "string".into();
    assert_eq!(
        registry.register(
            &context,
            Arc::new(DescriptionOnly(document(&changed, Support::Known))),
            None
        ),
        Err(Error::Invalid)
    );
    registry
        .register(&context, Arc::new(DescriptionOnly(inv.definition)), None)
        .unwrap();
    assert!(registry.select("cr2se.storage", 1, false).is_ok());
    assert!(matches!(
        registry.select("cr2se.storage", 1, true),
        Err(Error::Unsupported)
    ));
}
