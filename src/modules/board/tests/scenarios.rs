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
pub(super) struct Known;
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

// Readers racing complete publication never combine a new offering with an old definition.
#[test]
fn concurrent_reads_pin_complete_pairs() {
    use std::sync::Barrier;
    let id = IdentityId([1; 32]);
    let a = Arc::new(Authority::new());
    let p = Provider::new(id, a.clone(), Limits::default(), Arc::new(Known)).unwrap();
    let pg = a.grant(id, Action::Publish, 1_000_000).unwrap();
    let pc = Context {
        grant: &pg,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    };
    let rg = a.grant(id, Action::Read, 1_000_000).unwrap();
    let rc = Context { grant: &rg, ..pc };
    p.publish(&pc, prepare(id, 7, false)).unwrap();
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            for n in 0..20 {
                let mut b: serde_json::Value =
                    serde_json::from_str(&super::test_support::board(false, 7)).unwrap();
                let mut d: serde_json::Value =
                    serde_json::from_str(super::test_support::definition()).unwrap();
                let description = format!("Complete contract revision {n}.");
                b["providedServices"][0]["description"] = description.clone().into();
                d["description"] = description.into();
                let defs = BTreeMap::from([("fixture".into(), d.to_string().into_bytes())]);
                let next = Prepared::new(
                    id,
                    b.to_string().as_bytes(),
                    &defs,
                    Limits::default(),
                    &Known,
                )
                .unwrap();
                barrier.wait();
                p.publish(&pc, next).unwrap();
                barrier.wait();
            }
        });
        for _ in 0..20 {
            barrier.wait();
            let s = p.read(&rc).unwrap();
            let d = p.definition(&rc, &s, "fixture").unwrap();
            let b = super::offering::Board::parse(s.bytes(), Limits::default()).unwrap();
            assert_eq!(
                b.offerings["fixture"].description,
                d.document.value()["description"].as_str().unwrap()
            );
            barrier.wait();
        }
    });
}
// Revocation, wrong-purpose access and cancelled publication cannot change acknowledged state.
#[test]
fn revoked_and_cancelled_operations_have_no_effects() {
    let id = IdentityId([1; 32]);
    let a = Arc::new(Authority::new());
    let p = Provider::new(id, a.clone(), Limits::default(), Arc::new(Known)).unwrap();
    let g = a.grant(id, Action::Publish, 1_000_000).unwrap();
    let c = Context {
        grant: &g,
        deadline: Instant::now() + Duration::from_secs(10),
        cancelled: false,
    };
    let old = p.publish(&c, prepare(id, 7, false)).unwrap();
    assert!(matches!(p.read(&c), Err(Error::Denied)));
    let cancelled = Context {
        cancelled: true,
        ..c
    };
    assert!(matches!(
        p.publish(&cancelled, prepare(id, 9, false)),
        Err(Error::Cancelled)
    ));
    a.revoke();
    assert!(matches!(
        p.publish(&c, prepare(id, 9, false)),
        Err(Error::Denied)
    ));
    assert_eq!(old.bytes(), super::test_support::board(false, 7).as_bytes());
}

// A one-revision budget permits replacement once callers release old pins, without unbounded retention.
#[test]
fn unpinned_current_does_not_exhaust_single_revision_budget() {
    let id = IdentityId([1; 32]);
    let a = Arc::new(Authority::new());
    let l = Limits {
        snapshots: 1,
        ..Limits::default()
    };
    let p = Provider::new(id, a.clone(), l, Arc::new(Known)).unwrap();
    let g = a.grant(id, Action::Publish, 1_000_000).unwrap();
    let c = Context {
        grant: &g,
        deadline: Instant::now() + Duration::from_secs(10),
        cancelled: false,
    };
    let old = p.publish(&c, prepare(id, 7, false)).unwrap();
    assert!(matches!(
        p.publish(&c, prepare(id, 9, false)),
        Err(Error::Limit)
    ));
    drop(old);
    assert!(p.publish(&c, prepare(id, 9, false)).is_ok());
}
