//! Quote pricing receives independently validated metadata without inventing provider wire factors.
use super::{
    Error, assessment,
    authority::{Action, Authority},
    authorization,
    contract::{Context, Limits, OperationKey},
    handler::{Blob, Check, Peer, Requester},
    pricing::{self, Price},
    test_support::{
        family::{Echo, offering},
        selection::selected_ports,
    },
    validation::Semantics,
    value::Value,
};
use crate::modules::{
    board::{definition::Definition, offering::Offering},
    identity::api::IdentityId,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
struct MetadataGuard(Echo);
impl Semantics for MetadataGuard {
    // Retain the installed custom echo's schema restrictions independently of quote policy.
    fn validate(&self, d: &serde_json::Value, v: &Value) -> Result<(), Error> {
        self.0.validate(d, v)
    }
    // No nonfinite numerical semantics are introduced by metadata-aware quote validation.
    fn nonfinite(&self) -> bool {
        false
    }
}
impl Requester for MetadataGuard {
    // The complete standard-free custom definition remains the original independently installed contract.
    fn definition(&self) -> &Definition {
        self.0.definition()
    }
    // Fixed seven-credit economics remain unchanged by metadata-aware policy.
    fn terms(&self, o: &Offering) -> Result<(), Error> {
        self.0.terms(o)
    }
    // This fixture refuses an unwanted metadata proposal before acceptance while preserving the fixed quote price.
    fn pricing_for(&self, o: &Offering, metadata: &Value, factors: &Value) -> Result<Price, Error> {
        if metadata != &Value::String("allow".into()) || factors != &Value::Object(vec![]) {
            return Err(Error::Invalid);
        }
        self.pricing(o, factors)
    }
    // Requester checks retain the custom fixture's original private-expectation behavior.
    fn verify(
        &self,
        input: &Value,
        terms: &Blob,
        result: &Value,
        expectations: Option<&Value>,
    ) -> Result<Check, Error> {
        self.0.verify(input, terms, result, expectations)
    }
}
// The real public quote path must call metadata-aware family pricing after assessment, then enforce the same bound.
#[test]
fn quote_routes_validated_metadata_separately_from_factors() {
    let authority = Arc::new(Authority::new());
    let family = Arc::new(MetadataGuard(Echo::new()));
    let (_board, selection, _ba, registry) =
        selected_ports(false, family, Arc::new(Echo::new()), authority.clone());
    let local = IdentityId([2; 32]);
    let grant = authority.grant(local, Action::Invoke, 65536).unwrap();
    let context = Context {
        version: 1,
        grant: &grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let peer = Peer {
        local,
        remote: IdentityId([1; 32]),
        key: OperationKey {
            requester: selection.roles().0,
            connection: [9; 32],
            stream: 1,
        },
    };
    for (metadata, expected) in [
        ("allow", Ok(Price::Fixed(7))),
        ("deny", Err(Error::Invalid)),
    ] {
        let authorized = authorization::authorize(
            &context,
            &authority,
            local,
            &registry,
            selection.clone(),
            peer,
            None,
        )
        .unwrap();
        let input = Value::String(metadata.into());
        let assessed =
            assessment::assess(&context, &authority, authorized, &input, Limits::default())
                .unwrap();
        assert_eq!(assessed.metadata(), &input);
        assert_eq!(assessed.factors(), &Value::Object(vec![]));
        assert_eq!(pricing::quote(&assessed, 7), expected);
        if metadata == "allow" {
            assert_eq!(pricing::quote(&assessed, 6), Err(Error::Insufficient));
        }
    }
}
// Existing requesters retain their literal pricing even when a caller supplies metadata they do not use.
#[test]
fn legacy_pricing_default_stays_compatible() {
    let family = Echo::new();
    assert_eq!(
        family.pricing_for(
            &offering(false),
            &Value::Uint64(u64::MAX),
            &Value::Object(vec![])
        ),
        Ok(Price::Fixed(7))
    );
}
