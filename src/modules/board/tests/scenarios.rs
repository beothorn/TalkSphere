//! Caller-oriented Board scenarios use real parsing and public owner/cache contracts.
use super::{
    Error,
    authority::{Action, Authority},
    cache::{Cache, Provenance},
    config::Limits,
    contract::Context,
    offering::Offering,
    prepared::Prepared,
    selection::Request,
    semantics::Capabilities,
    state::Provider,
};
use crate::modules::identity::api::IdentityId;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
struct Known;
impl Capabilities for Known {
    // Exact custom fixture support is injected explicitly rather than installed by production CLI.
    fn understands(&self, o: &Offering) -> bool {
        o.service == "org.example.echo" && o.version == 1
    }
    // The fixture models only positive fixed-price whole operations.
    fn terms(&self, o: &Offering) -> Result<(), Error> {
        if o.source
            .get("price")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|p| p > 0)
        {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
    }
    // Unknown names are required unsupported semantics, never silently satisfied.
    fn precondition(&self, name: &str, authenticated: bool) -> bool {
        name == "cr2se.identity" && authenticated
    }
    // Both directions are supported only in this controlled composition.
    fn local(&self, _: &Offering) -> bool {
        true
    }
}
// Create a complete isolated publication from literal specification-derived fixture documents.
fn prepare(id: IdentityId, price: u64, wanted: bool) -> Prepared {
    let defs = BTreeMap::from([(
        "fixture".into(),
        super::test_support::definition().as_bytes().to_vec(),
    )]);
    Prepared::new(
        id,
        super::test_support::board(wanted, price).as_bytes(),
        &defs,
        Limits::default(),
        &Known,
    )
    .unwrap()
}
// A price change blocks future use while copied terms remain immutable after removal and owner drop.
#[test]
fn repricing_and_removal_preserve_retained_terms() {
    let id = IdentityId([1; 32]);
    let a = Arc::new(Authority::new());
    let p = Provider::new(id, a.clone(), Limits::default(), Arc::new(Known)).unwrap();
    let pg = a.grant(id, Action::Publish, 1_000_000).unwrap();
    let pc = Context {
        grant: &pg,
        deadline: Instant::now() + Duration::from_secs(10),
        cancelled: false,
    };
    let sg = a.grant(id, Action::Select, 1_000_000).unwrap();
    let sc = Context { grant: &sg, ..pc };
    let r = Request {
        id: "fixture",
        service: "org.example.echo",
        version: 1,
        description: "Return the supplied label.",
        other: IdentityId([2; 32]),
        authenticated: true,
    };
    let first = p.publish(&pc, prepare(id, 7, false)).unwrap();
    let selected = p.select(&sc, &first, &r).unwrap();
    assert_eq!(p.current_selection(&sc, &selected), Ok(()));
    p.publish(&pc, prepare(id, 9, false)).unwrap();
    assert_eq!(p.current_selection(&sc, &selected), Err(Error::Stale));
    assert!(matches!(p.select(&sc, &first, &r), Err(Error::Stale)));
    let empty = Prepared::new(
        id,
        br#"{"version":1,"providedServices":[],"wantedServices":[]}"#,
        &BTreeMap::new(),
        Limits::default(),
        &Known,
    )
    .unwrap();
    p.publish(&pc, empty).unwrap();
    assert_eq!(p.current_selection(&sc, &selected), Err(Error::Stale));
    drop(p);
    assert_eq!(selected.offering().source["price"].as_u64(), Some(7));
}
// Direct remote selection binds exact roles; forwarding identical content cannot authenticate a publisher.
#[test]
fn remote_selection_requires_direct_provenance() {
    let id = IdentityId([1; 32]);
    let a = Arc::new(Authority::new());
    let mut cache = Cache::new(a.clone(), Limits::default()).unwrap();
    let cg = a.grant(id, Action::Cache, 1_000_000).unwrap();
    let cc = Context {
        grant: &cg,
        deadline: Instant::now() + Duration::from_secs(10),
        cancelled: false,
    };
    let sg = a.grant(id, Action::Select, 1_000_000).unwrap();
    let sc = Context { grant: &sg, ..cc };
    let r = Request {
        id: "fixture",
        service: "org.example.echo",
        version: 1,
        description: "Return the supplied label.",
        other: IdentityId([2; 32]),
        authenticated: true,
    };
    for wanted in [false, true] {
        let bytes = super::test_support::board(wanted, 7);
        let remote = cache
            .store(&cc, Provenance::Direct(id), bytes.as_bytes(), 100)
            .unwrap();
        let remote = cache
            .attach(
                &cc,
                &remote,
                "fixture",
                super::test_support::definition().as_bytes(),
                100,
            )
            .unwrap();
        let selected = cache.select(&sc, &remote, &r, &Known, 101).unwrap();
        assert_eq!(selected.roles().0, if wanted { id } else { r.other });
        let forwarded = cache
            .store(&cc, Provenance::Forwarded(id), bytes.as_bytes(), 100)
            .unwrap();
        assert!(matches!(
            cache.select(&sc, &forwarded, &r, &Known, 101),
            Err(Error::Unsupported)
        ));
    }
}
