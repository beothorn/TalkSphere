//! Quote pricing receives independently validated metadata without inventing provider wire factors.
use super::{
    Error, assessment,
    authority::{Action, Authority},
    authorization,
    contract::{Context, Limits, OperationKey},
    handler::{Assessment, Blob, Check, Peer, Requester},
    pricing::{self, Price},
    test_support::{
        family::{Echo, offering},
        selection::selected_ports,
    },
    validation::Semantics,
    value::{Budget, Value},
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

struct PeerGuard {
    echo: Echo,
    expected: Peer,
}
impl PeerGuard {
    // Check every original association field; payload/issuer information cannot substitute the authenticated peer or attempt.
    fn association(&self, peer: Peer) -> Result<(), Error> {
        if peer.local != self.expected.local
            || peer.remote != self.expected.remote
            || peer.key != self.expected.key
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
}
impl Semantics for PeerGuard {
    // Keep independent installed schema restrictions while testing context delivery.
    fn validate(&self, d: &serde_json::Value, v: &Value) -> Result<(), Error> {
        self.echo.validate(d, v)
    }
    // The echo fixture introduces no nonfinite values.
    fn nonfinite(&self) -> bool {
        false
    }
}
impl Requester for PeerGuard {
    // The original custom contract remains the independently registered executable fixture.
    fn definition(&self) -> &Definition {
        self.echo.definition()
    }
    // Economics remain the same fixed seven-credit custom contract.
    fn terms(&self, o: &Offering) -> Result<(), Error> {
        self.echo.terms(o)
    }
    // Context-free pricing is intentionally unavailable so quote routing cannot silently bypass the new association.
    fn pricing_for(&self, _: &Offering, _: &Value, _: &Value) -> Result<Price, Error> {
        Err(Error::Unsupported)
    }
    // Prove quote calculation supplies the exact original trusted association in both economic directions.
    fn pricing_with_peer(
        &self,
        peer: Peer,
        o: &Offering,
        _: &Value,
        factors: &Value,
    ) -> Result<Price, Error> {
        self.association(peer)?;
        self.echo.pricing(o, factors)
    }
    // Retain input metadata only after checking the explicit selected peer context.
    fn assessment_metadata_with_peer(
        &self,
        peer: Peer,
        _: &Offering,
        input: &Value,
        budget: Budget,
    ) -> Result<Value, Error> {
        self.association(peer)?;
        input.bounded(budget)?;
        Ok(input.clone())
    }
    // Validate remote expectations through the new association-aware port before the old value rules run.
    fn validate_assessment_with_peer(
        &self,
        peer: Peer,
        _: &Offering,
        input: &Value,
        metadata: &Value,
        factors: &Value,
        budget: Budget,
    ) -> Result<(), Error> {
        self.association(peer)?;
        self.echo
            .validate_assessment(input, metadata, factors, budget)
    }
    // Preserve original private echo verification independently of quote association policy.
    fn verify(
        &self,
        input: &Value,
        terms: &Blob,
        result: &Value,
        expectations: Option<&Value>,
    ) -> Result<Check, Error> {
        self.echo.verify(input, terms, result, expectations)
    }
}
// Real local-provider and remote-provider assessments pass trusted roles/attempts to pricing while preserving explicit spending bounds.
#[test]
fn authenticated_peer_reaches_metadata_validation_and_quote() {
    for wanted in [false, true] {
        let authority = Arc::new(Authority::new());
        let local = IdentityId([2; 32]);
        let remote = IdentityId([1; 32]);
        let peer = Peer {
            local,
            remote,
            key: OperationKey {
                requester: if wanted { local } else { remote },
                connection: [8; 32],
                stream: 7,
            },
        };
        let family = Arc::new(PeerGuard {
            echo: Echo::new(),
            expected: peer,
        });
        let (_board, selection, _ba, registry) = selected_ports(
            wanted,
            family.clone(),
            Arc::new(Echo::new()),
            authority.clone(),
        );
        let grant = authority
            .grant(
                local,
                if wanted {
                    Action::Request
                } else {
                    Action::Invoke
                },
                65536,
            )
            .unwrap();
        let context = Context {
            version: 1,
            grant: &grant,
            deadline: Instant::now() + Duration::from_secs(60),
            cancelled: false,
        };
        let input = Value::String("provider-looking payload is only data".into());
        let authorized = authorization::authorize(
            &context,
            &authority,
            local,
            &registry,
            selection.clone(),
            peer,
            if wanted { Some(input.clone()) } else { None },
        )
        .unwrap();
        assert_eq!(authorized.peer().key, peer.key);
        let metadata = family
            .assessment_metadata_with_peer(
                authorized.peer(),
                selection.offering(),
                &input,
                Budget {
                    depth: 8,
                    nodes: 128,
                    bytes: 4096,
                },
            )
            .unwrap();
        let assessed = if wanted {
            assessment::remote(
                &context,
                &authority,
                authorized,
                Assessment {
                    metadata,
                    factors: Value::Object(vec![]),
                    input_bytes: 4096,
                    pin: None,
                },
                Limits::default(),
            )
            .unwrap()
        } else {
            assessment::assess(
                &context,
                &authority,
                authorized,
                &metadata,
                Limits::default(),
            )
            .unwrap()
        };
        assert_eq!(pricing::quote(&assessed, 7), Ok(Price::Fixed(7)));
        assert_eq!(pricing::quote(&assessed, 6), Err(Error::Insufficient));
        let forged = Peer {
            key: OperationKey {
                connection: [9; 32],
                ..peer.key
            },
            ..peer
        };
        assert_eq!(
            family.pricing_with_peer(forged, selection.offering(), &input, &Value::Object(vec![])),
            Err(Error::Denied)
        );
    }
}
