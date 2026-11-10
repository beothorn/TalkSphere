//! Exact provider quote bytes, scoped cancellation and pre-acceptance failure paths over real dependencies.
use super::{
    Error,
    authority::Action,
    contract::Limits,
    message::Message,
    proposal::Authorizer,
    quote::{Entropy, Issuer},
    test_support::world::{World, context, service_context},
};
use crate::modules::{
    board,
    services::{self, pricing::Price, quote::BoardCurrent, value::Value},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
struct Random(Mutex<VecDeque<Result<[u8; 32], Error>>>);
impl Entropy for Random {
    // Exhausted/failing CSPRNG replies never derive a quote ID from inputs, clocks or a fixed fallback.
    fn quote_id(&self) -> Result<[u8; 32], Error> {
        self.0
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(Error::Unavailable))
    }
}
// Quotes bind complete current terms, exact original bytes/hash and random ID, while leaving real credits unreserved.
#[test]
fn exact_provider_quote_and_scoped_release() {
    let w = World::new(false);
    let pg = w
        .authority
        .grant(w.local, Action::Invoke, 16_777_216)
        .unwrap();
    let sg = w
        .services
        .grant(w.local, services::authority::Action::Invoke, 16_777_216)
        .unwrap();
    let c = context(&pg);
    let sc = service_context(&sg);
    let bg = w
        .board_authority
        .grant(w.local, board::authority::Action::Select, 16_777_216)
        .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: c.deadline,
        cancelled: false,
    };
    let current = BoardCurrent {
        owner: &w.board,
        context: &bc,
    };
    let auth = Authorizer::new(
        w.local,
        w.authority.clone(),
        w.services.clone(),
        w.registry.clone(),
        Limits::default(),
        w.gate.clone(),
    )
    .unwrap();
    let random = Arc::new(Random(Mutex::new(VecDeque::from([
        Ok([7; 32]),
        Ok([8; 32]),
    ]))));
    let issuer = Issuer::new(
        w.local,
        w.authority.clone(),
        w.services.clone(),
        Limits::default(),
        services::contract::Limits {
            quotes: 1,
            ..Default::default()
        },
        w.gate.clone(),
        random,
    )
    .unwrap();
    let (a, s, m) = w.proposal(1, &Value::String("SECRET_BODY".into()));
    let offering = s.offering().source.clone();
    let proposal = auth.authorize(&c, &sc, a, s, &m).unwrap();
    let q = issuer
        .issue(
            &c,
            &sc,
            proposal,
            &current,
            c.deadline + Duration::from_secs(100),
        )
        .unwrap();
    let wire = Message::parse(q.bytes(), 65536).unwrap();
    assert_eq!(wire.value()["maximumPrice"], "7");
    assert_eq!(wire.value()["offering"], offering);
    assert_eq!(wire.value()["priceMode"], "fixed");
    assert_eq!(wire.value()["pricingInputs"], serde_json::json!({}));
    assert_eq!(
        super::base64::decode(wire.value()["quoteId"].as_str().unwrap(), 32).unwrap(),
        vec![7; 32]
    );
    assert_eq!(q.id(), [7; 32]);
    assert_eq!(q.price(), Price::Fixed(7));
    assert_eq!(q.association(), a);
    assert_eq!(q.expires(), c.deadline.min(sc.deadline));
    assert!(q.input().descriptors().is_empty());
    assert!(q.payment().transfer.is_none());
    use sha2::{Digest, Sha256};
    assert_eq!(q.hash(), <[u8; 32]>::from(Sha256::digest(q.bytes())));
    assert!(!String::from_utf8_lossy(q.bytes()).contains("SECRET_BODY"));
    assert_eq!(w.available(), 100);
    assert_eq!(w.family.executions.load(Ordering::SeqCst), 0);
    let (a, s, m) = w.proposal(2, &Value::String("input".into()));
    let proposal = auth.authorize(&c, &sc, a, s, &m).unwrap();
    assert!(matches!(
        issuer.issue(&c, &sc, proposal, &current, c.deadline),
        Err(Error::Limit)
    ));
    issuer.discard(&c, &sc, &q).unwrap();
    // A discarded or accepted original handle never cancels durable work; only its unaccepted slot is released.
    assert_eq!(issuer.discard(&c, &sc, &q), Ok(()));
    assert_eq!(w.available(), 100);
}
// Entropy failure and inconsistent registered price callbacks cannot leak a slot, reserve credits or execute work.
#[test]
fn prequote_dependency_failures() {
    let w = World::new(false);
    let pg = w
        .authority
        .grant(w.local, Action::Invoke, 16_777_216)
        .unwrap();
    let sg = w
        .services
        .grant(w.local, services::authority::Action::Invoke, 16_777_216)
        .unwrap();
    let c = context(&pg);
    let sc = service_context(&sg);
    let bg = w
        .board_authority
        .grant(w.local, board::authority::Action::Select, 16_777_216)
        .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: c.deadline,
        cancelled: false,
    };
    let current = BoardCurrent {
        owner: &w.board,
        context: &bc,
    };
    let auth = Authorizer::new(
        w.local,
        w.authority.clone(),
        w.services.clone(),
        w.registry.clone(),
        Limits::default(),
        w.gate.clone(),
    )
    .unwrap();
    let issuer = Issuer::new(
        w.local,
        w.authority.clone(),
        w.services.clone(),
        Limits::default(),
        services::contract::Limits {
            quotes: 1,
            ..Default::default()
        },
        w.gate.clone(),
        Arc::new(Random(Mutex::new(VecDeque::from([
            Err(Error::Unavailable),
            Ok([1; 32]),
            Ok([2; 32]),
        ])))),
    )
    .unwrap();
    let make = || {
        let (a, s, m) = w.proposal(1, &Value::String("input".into()));
        let mut raw = m.value().clone();
        raw["maximumPrice"] = serde_json::json!("9");
        let m = Message::parse(raw.to_string().as_bytes(), 65536).unwrap();
        auth.authorize(&c, &sc, a, s, &m).unwrap()
    };
    assert!(matches!(
        issuer.issue(&c, &sc, make(), &current, c.deadline),
        Err(Error::Unavailable)
    ));
    w.family.script_prices(&[7, 8]);
    assert!(matches!(
        issuer.issue(&c, &sc, make(), &current, c.deadline),
        Err(Error::Conflict)
    ));
    let q = issuer.issue(&c, &sc, make(), &current, c.deadline).unwrap();
    assert_eq!(q.price(), Price::Fixed(7));
    issuer.discard(&c, &sc, &q).unwrap();
    assert_eq!(w.available(), 100);
    assert_eq!(w.family.executions.load(Ordering::SeqCst), 0);
}
