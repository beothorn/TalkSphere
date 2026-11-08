//! Registration, reserved namespace, Board semantics and retained recovery evidence.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Context, Limits},
    handler::Requester,
    registry::{NoStandards, Registry},
    test_support::family::{Echo, offering},
};
use crate::modules::{
    board::semantics::{Capabilities, assess},
    identity::api::IdentityId,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Exact custom registration supports independent requester and provider roles and drains without losing its pin.
#[test]
fn registry_board_and_drain() {
    let local = IdentityId([2; 32]);
    let a = Arc::new(Authority::new());
    let register = a.grant(local, Action::Register, 65536).unwrap();
    let recover = a.grant(local, Action::Recover, 65536).unwrap();
    let c = Context {
        version: 1,
        grant: &register,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let r = Registry::new(local, a.clone(), Limits::default(), Arc::new(NoStandards)).unwrap();
    let f = Arc::new(Echo::new());
    r.register(&c, f.clone(), Some(f.clone())).unwrap();
    assert_eq!(r.register(&c, f.clone(), None), Err(Error::Conflict));
    assert!(r.select("org.example.echo", 2, false).is_err());
    let o = offering(false);
    assert_eq!(assess(&r, &o, f.definition(), true, true), Ok(()));
    assert!(r.precondition("cr2se.identity", true));
    assert!(!r.precondition("unknown", true));
    let pinned = r.select(&o.service, 1, true).unwrap();
    assert!(pinned.provider().is_ok());
    r.drain(&c, &o.service, 1).unwrap();
    assert!(!r.understands(&o));
    assert!(!r.local(&o));
    assert!(r.select(&o.service, 1, false).is_err());
    assert!(pinned.provider().is_ok());
    let recovery = Context {
        grant: &recover,
        ..c
    };
    assert!(r.recover(&recovery, &o.service, 1).is_ok());
    assert!(matches!(r.recover(&c, &o.service, 1), Err(Error::Denied)));
}
// Requester-only support enables wanted contracts while unknown definitions, authority and capacity fail closed.
#[test]
fn requester_only_and_refusals() {
    let local = IdentityId([2; 32]);
    let a = Arc::new(Authority::new());
    let g = a.grant(local, Action::Register, 65536).unwrap();
    let c = Context {
        version: 1,
        grant: &g,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let r = Registry::new(
        local,
        a.clone(),
        Limits {
            handlers: 1,
            ..Limits::default()
        },
        Arc::new(NoStandards),
    )
    .unwrap();
    let f = Arc::new(Echo::new());
    r.register(&c, f.clone(), None).unwrap();
    assert!(r.local(&offering(true)));
    assert!(!r.local(&offering(false)));
    assert!(
        r.select("org.example.echo", 1, false)
            .unwrap()
            .provider()
            .is_err()
    );
    let mut wrong = f.definition().clone();
    let mut v = wrong.document.value().clone();
    v["input"]["type"] = "bool".into();
    wrong.document = crate::modules::board::document::Document::parse(
        v.to_string().as_bytes(),
        Default::default(),
    )
    .unwrap();
    assert!(r.contract(&offering(false), &wrong).is_err());
    let foreign = Authority::new();
    let fg = foreign.grant(local, Action::Register, 65536).unwrap();
    assert_eq!(
        r.register(&Context { grant: &fg, ..c }, f.clone(), None),
        Err(Error::Denied)
    );
    a.revoke();
    assert_eq!(r.drain(&c, "org.example.echo", 1), Err(Error::Denied));
}
