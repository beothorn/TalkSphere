//! Provided proposals use actual Registry and Ledger policy without spending or executing before consent.
use super::{
    Error,
    admission::{LedgerGate, Roles},
    authority::{Action, Authority},
    contract::Limits,
    message::Message,
    proposal::{Authorizer, Mode},
    test_support::world::{World, context, ledger_context, service_context},
};
use crate::modules::{
    identity::{api::IdentityId, text},
    ledger,
    services::{
        self,
        value::{Budget, Value},
    },
};
use std::{collections::BTreeMap, sync::atomic::Ordering};
// Exact selected roles and metadata authorize a proposal but create neither a hold nor a journal operation.
#[test]
fn provided_authorization_has_no_effects() {
    let w = World::new(false);
    let incoming = w
        .authority
        .grant(w.local, Action::Invoke, 16_777_216)
        .unwrap();
    let sg = w
        .services
        .grant(w.local, services::authority::Action::Invoke, 16_777_216)
        .unwrap();
    let auth = Authorizer::new(
        w.local,
        w.authority.clone(),
        w.services.clone(),
        w.registry.clone(),
        Limits::default(),
        w.gate.clone(),
    )
    .unwrap();
    let (a, s, m) = w.proposal(1, &Value::String("input".into()));
    let p = auth
        .authorize(&context(&incoming), &service_context(&sg), a, s, &m)
        .unwrap();
    assert_eq!(p.mode(), Mode::Invoke);
    assert_eq!(p.maximum(), 7);
    assert!(p.payment().transfer.is_none());
    assert_eq!(
        p.authorized().selection().roles(),
        (w.requester, w.local, w.local)
    );
    assert!(p.input().descriptors().is_empty());
    assert_eq!(
        p.input().assemble(&BTreeMap::new()),
        Ok(Value::String("input".into()))
    );
    assert_eq!(w.family.assessments.load(Ordering::SeqCst), 0);
    assert_eq!(w.family.executions.load(Ordering::SeqCst), 0);
    assert_eq!(w.available(), 100);
    let observe = w
        .services
        .grant(w.local, services::authority::Action::Observe, 16_777_216)
        .unwrap();
    let key = services::contract::OperationKey {
        requester: w.requester,
        connection: [8; 32],
        stream: 1,
    };
    assert_eq!(w.runtime.status(&service_context(&observe), key), Ok(None));
}
// Wrong identities, contracts, values, actions, unsupported local checks and zero bounds fail before callbacks or financial mutation.
#[test]
fn refusal_matrix() {
    let w = World::new(false);
    let incoming = w
        .authority
        .grant(w.local, Action::Invoke, 16_777_216)
        .unwrap();
    let sg = w
        .services
        .grant(w.local, services::authority::Action::Invoke, 16_777_216)
        .unwrap();
    let auth = Authorizer::new(
        w.local,
        w.authority.clone(),
        w.services.clone(),
        w.registry.clone(),
        Limits::default(),
        w.gate.clone(),
    )
    .unwrap();
    let (a, s, m) = w.proposal(1, &Value::String("input".into()));
    for (field, value) in [
        ("publisherId", serde_json::json!(text::format(w.requester))),
        ("creditIssuer", serde_json::json!(text::format(w.requester))),
        ("service", serde_json::json!("other")),
        ("serviceVersion", serde_json::json!(2)),
        ("input", serde_json::json!(false)),
        ("maximumPrice", serde_json::json!("0")),
        ("mode", serde_json::json!("check")),
    ] {
        let mut raw = m.value().clone();
        raw[field] = value;
        let bad = Message::parse(raw.to_string().as_bytes(), 65536).unwrap();
        assert!(
            auth.authorize(
                &context(&incoming),
                &service_context(&sg),
                a,
                s.clone(),
                &bad
            )
            .is_err()
        );
    }
    let foreign = Authority::new();
    let grant = foreign.grant(w.local, Action::Invoke, 16_777_216).unwrap();
    assert!(matches!(
        auth.authorize(&context(&grant), &service_context(&sg), a, s.clone(), &m),
        Err(Error::Denied)
    ));
    let admin = w
        .ledger_authority
        .grant(w.local, ledger::authority::Action::Administer, 16_777_216)
        .unwrap();
    w.ledger
        .set_trust(
            &ledger_context(&admin),
            w.requester,
            ledger::accounting::Trust::default(),
        )
        .unwrap();
    assert!(matches!(
        auth.authorize(&context(&incoming), &service_context(&sg), a, s.clone(), &m),
        Err(Error::Denied)
    ));
    let register = w
        .services
        .grant(w.local, services::authority::Action::Register, 16_777_216)
        .unwrap();
    w.registry
        .drain(&service_context(&register), &s.offering().service, 1)
        .unwrap();
    assert!(matches!(
        auth.authorize(&context(&incoming), &service_context(&sg), a, s, &m),
        Err(Error::Unsupported)
    ));
    assert_eq!(w.family.assessments.load(Ordering::SeqCst), 0);
    assert_eq!(w.family.executions.load(Ordering::SeqCst), 0);
    assert_eq!(w.available(), 100);
}
// Large bytes are assessed only as declared lengths through the new public Services metadata contract, never fake content.
#[test]
fn unresolved_metadata_before_body() {
    let w = World::new(true);
    let incoming = w
        .authority
        .grant(w.local, Action::Invoke, 16_777_216)
        .unwrap();
    let sg = w
        .services
        .grant(w.local, services::authority::Action::Invoke, 16_777_216)
        .unwrap();
    let auth = Authorizer::new(
        w.local,
        w.authority.clone(),
        w.services.clone(),
        w.registry.clone(),
        Limits::default(),
        w.gate.clone(),
    )
    .unwrap();
    let (a, s, m) = w.proposal(1, &Value::Bytes(vec![7; 4097]));
    let p = auth
        .authorize(&context(&incoming), &service_context(&sg), a, s, &m)
        .unwrap();
    let input = p.input.describe().unwrap();
    let budget = Budget {
        depth: 31,
        nodes: 65536,
        bytes: 16_777_216,
    };
    assert_eq!(input.complete(budget), Err(services::Error::Unsupported));
    let assessed = services::assessment::assess_metadata(
        &service_context(&sg),
        &w.services,
        p.authorized,
        &input,
        Default::default(),
    )
    .unwrap();
    assert_eq!(assessed.input_bytes(), 4097);
    assert_eq!(w.family.assessments.load(Ordering::SeqCst), 1);
    assert_eq!(w.family.executions.load(Ordering::SeqCst), 0);
    assert_eq!(w.available(), 100);
}
// Provider issuer-risk policy and finite capacities are read from the actual Ledger and never reserve during quoting.
#[test]
fn read_only_ledger_policy() {
    let w = World::new(false);
    let g = w
        .authority
        .grant(w.local, Action::Invoke, 16_777_216)
        .unwrap();
    let c = context(&g);
    let roles = Roles {
        requester: w.requester,
        provider: w.local,
        issuer: w.local,
    };
    assert_eq!(w.gate.check(&c, roles), Ok(()));
    assert_eq!(w.gate.capacity(&c, roles, 100, 0), Ok(()));
    assert_eq!(w.gate.capacity(&c, roles, 101, 0), Err(Error::Insufficient));
    assert_eq!(w.gate.capacity(&c, roles, u64::MAX, 1), Err(Error::Limit));
    assert_eq!(
        w.gate.check(
            &c,
            Roles {
                issuer: w.requester,
                ..roles
            }
        ),
        Err(Error::Unsupported)
    );
    assert_eq!(
        w.gate.check(
            &c,
            Roles {
                issuer: IdentityId([3; 32]),
                ..roles
            }
        ),
        Err(Error::Unsupported)
    );
    let read = w
        .ledger_authority
        .grant(w.local, ledger::authority::Action::Read, 16_777_216)
        .unwrap();
    assert!(matches!(
        LedgerGate::new(
            IdentityId([3; 32]),
            w.ledger.clone(),
            read,
            Default::default()
        ),
        Err(Error::Denied)
    ));
    assert_eq!(w.available(), 100);
}
