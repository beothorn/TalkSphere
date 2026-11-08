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

struct Alternate {
    base: Echo,
    definition: crate::modules::board::definition::Definition,
}
impl super::validation::Semantics for Alternate {
    // Alternate fixture explicitly preserves the installed echo value semantics.
    fn validate(&self, s: &serde_json::Value, v: &super::value::Value) -> Result<(), Error> {
        self.base.validate(s, v)
    }
    // No arbitrary metadata enables nonfinite values.
    fn nonfinite(&self) -> bool {
        false
    }
}
impl Requester for Alternate {
    // A literal alternate name exercises capacity and reserved namespace checks.
    fn definition(&self) -> &crate::modules::board::definition::Definition {
        &self.definition
    }
    // Economic validation remains the independently installed fixture rule.
    fn terms(&self, o: &crate::modules::board::offering::Offering) -> Result<(), Error> {
        self.base.terms(o)
    }
    // Check outcomes remain local even when registry admission rejects a description.
    fn verify(
        &self,
        i: &super::value::Value,
        t: &super::handler::Blob,
        r: &super::value::Value,
        e: Option<&super::value::Value>,
    ) -> Result<super::handler::Check, Error> {
        self.base.verify(i, t, r, e)
    }
}
// Reserved names require a catalogue; finite registration capacity never evicts accepted implementations.
#[test]
fn catalogue_and_capacity() {
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
        a,
        Limits {
            handlers: 1,
            ..Limits::default()
        },
        Arc::new(NoStandards),
    )
    .unwrap();
    let alternate = |name: &str| {
        let base = Echo::new();
        let mut definition = base.definition().clone();
        let mut v = definition.document.value().clone();
        v["service"] = name.into();
        definition.document = crate::modules::board::document::Document::parse(
            v.to_string().as_bytes(),
            Default::default(),
        )
        .unwrap();
        Arc::new(Alternate { base, definition })
    };
    assert_eq!(
        r.register(&c, alternate("cr2se.storage"), None),
        Err(Error::Unsupported)
    );
    r.register(&c, Arc::new(Echo::new()), None).unwrap();
    assert_eq!(
        r.register(&c, alternate("org.example.other"), None),
        Err(Error::Limit)
    );
    assert!(r.select("org.example.echo", 1, false).is_ok());
}
