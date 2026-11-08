//! Real Board publication and immutable selection through installed Services capabilities.
use super::super::{
    authority::{Action, Authority},
    contract::{Context, Limits},
    handler::{Provider as FamilyProvider, Requester},
    registry::{NoStandards, Registry},
};
use super::family::{Echo, offering};
use crate::modules::{
    board::{self, selection::Selection, state::Provider},
    identity::api::IdentityId,
};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
// The default fixture installs a literal valid echo provider.
pub(in crate::modules::services) fn selected(
    wanted: bool,
) -> (
    Provider,
    Selection,
    Arc<board::authority::Authority>,
    Arc<Registry>,
) {
    selected_with_family(wanted, Arc::new(Echo::new()), Arc::new(Authority::new()))
}
// Publish a complete real Board and return its owner plus selected immutable terms; no private tables are read.
pub(in crate::modules::services) fn selected_with_family(
    wanted: bool,
    f: Arc<Echo>,
    authority: Arc<Authority>,
) -> (
    Provider,
    Selection,
    Arc<board::authority::Authority>,
    Arc<Registry>,
) {
    selected_ports(wanted, f.clone(), f, authority)
}
// Compose explicitly supplied requester/provider ports with default literal economic roles.
pub(in crate::modules::services) fn selected_ports(
    wanted: bool,
    f: Arc<dyn Requester>,
    provider: Arc<dyn FamilyProvider>,
    a: Arc<Authority>,
) -> (
    Provider,
    Selection,
    Arc<board::authority::Authority>,
    Arc<Registry>,
) {
    selected_terms(
        wanted,
        f,
        Some(provider),
        a,
        IdentityId([2; 32]),
        IdentityId([1; 32]),
    )
}
// Select wanted work with a requester codec and genuinely absent local provider.
pub(in crate::modules::services) fn selected_requester(
    f: Arc<dyn Requester>,
    a: Arc<Authority>,
) -> (
    Provider,
    Selection,
    Arc<board::authority::Authority>,
    Arc<Registry>,
) {
    selected_terms(true, f, None, a, IdentityId([2; 32]), IdentityId([1; 32]))
}
// Exact issuer/other identities are supplied explicitly to exercise independent settlement modes.
pub(in crate::modules::services) fn selected_terms(
    wanted: bool,
    f: Arc<dyn Requester>,
    provider: Option<Arc<dyn FamilyProvider>>,
    a: Arc<Authority>,
    issuer: IdentityId,
    other: IdentityId,
) -> (
    Provider,
    Selection,
    Arc<board::authority::Authority>,
    Arc<Registry>,
) {
    let publisher = IdentityId([2; 32]);
    let g = a.grant(publisher, Action::Register, 65536).unwrap();
    let context = Context {
        version: 1,
        grant: &g,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let r =
        Arc::new(Registry::new(publisher, a, Limits::default(), Arc::new(NoStandards)).unwrap());
    r.register(&context, f.clone(), provider).unwrap();
    let ba = Arc::new(board::authority::Authority::new());
    let p = Provider::new(publisher, ba.clone(), Default::default(), r.clone()).unwrap();
    let publish = ba
        .grant(publisher, board::authority::Action::Publish, 65536)
        .unwrap();
    let select = ba
        .grant(publisher, board::authority::Action::Select, 65536)
        .unwrap();
    let c = board::contract::Context {
        grant: &publish,
        deadline: context.deadline,
        cancelled: false,
    };
    let mut o = offering(wanted).source;
    o["creditIssuer"] = crate::modules::identity::text::format(issuer).into();
    let raw = if wanted {
        serde_json::json!({"version":1,"providedServices":[],"wantedServices":[o]})
    } else {
        serde_json::json!({"version":1,"providedServices":[o],"wantedServices":[]})
    };
    let candidate = board::prepared::Prepared::new(
        publisher,
        raw.to_string().as_bytes(),
        &BTreeMap::from([("echo".into(), f.definition().document.bytes().to_vec())]),
        Default::default(),
        r.as_ref(),
    )
    .unwrap();
    let snapshot = p.publish(&c, candidate).unwrap();
    let selected = p
        .select(
            &board::contract::Context {
                grant: &select,
                ..c
            },
            &snapshot,
            &board::selection::Request {
                id: "echo",
                service: "org.example.echo",
                version: 1,
                description: "Return the original text.",
                other,
                authenticated: true,
            },
        )
        .unwrap();
    (p, selected, ba, r)
}
