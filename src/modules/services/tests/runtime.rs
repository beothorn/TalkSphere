//! Real Board, Services and Ledger admission through public contracts.
use super::{
    Error, accounting,
    assessment::assess,
    authority::{Action, Authority, Grant},
    authorization::authorize,
    contract::{Context, Limits, OperationKey},
    handler::{Blob, Peer},
    persistence::Memory,
    quote::{Accepted, BoardCurrent, Issue, Quoter},
    runtime::{Config, Coordinator, Payment, Phase},
    test_support::selection::selected,
    value::Value,
};
use crate::modules::{board, identity::api::IdentityId, ledger};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Call contexts are explicitly short-lived and never originate in peer payloads.
fn context(grant: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    }
}
// Build exact accepted consent through a real Board and trusted original-stream quote, without network claims.
fn accepted(
    a: Arc<Authority>,
    registry: &super::registry::Registry,
    stream: u32,
    bytes: u8,
) -> Accepted {
    let (owner, s, ba, _) = selected(false);
    let local = IdentityId([2; 32]);
    let g = a.grant(local, Action::Invoke, 65536).unwrap();
    let c = context(&g);
    let peer = Peer {
        local,
        remote: IdentityId([1; 32]),
        key: OperationKey {
            requester: s.roles().0,
            connection: [8; 32],
            stream,
        },
    };
    let auth = authorize(&c, &a, local, registry, s, peer, None).unwrap();
    let assessed = assess(
        &c,
        &a,
        auth,
        &Value::String("input".into()),
        Limits::default(),
    )
    .unwrap();
    let q = Quoter::new(local, a, Limits::default()).unwrap();
    let quote = q
        .issue(
            &c,
            Issue {
                assessed,
                bytes: Blob::new(vec![bytes], 1).unwrap(),
                maximum: 7,
                expires: c.deadline,
            },
        )
        .unwrap();
    let bg = ba
        .grant(local, board::authority::Action::Select, 65536)
        .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: c.deadline,
        cancelled: false,
    };
    q.accept(
        &c,
        &quote,
        peer,
        quote.hash(),
        &BoardCurrent {
            owner: &owner,
            context: &bc,
        },
    )
    .unwrap()
}
// Real finite provider-issued credits must be held once before any handler execution.
#[test]
fn durable_binding_and_real_reservation() {
    let local = IdentityId([2; 32]);
    let requester = IdentityId([1; 32]);
    let a = Arc::new(Authority::new());
    let init = a.grant(local, Action::Register, 1_000_000).unwrap();
    let coordinate = a.grant(local, Action::Coordinate, 1_000_000).unwrap();
    let observe = a.grant(local, Action::Observe, 1_000_000).unwrap();
    let la = Arc::new(ledger::authority::Authority::new());
    let admin = la
        .grant(local, ledger::authority::Action::Administer, 1_000_000)
        .unwrap();
    let lc = ledger::contract::Context {
        version: 1,
        grant: &admin,
        deadline: context(&init).deadline,
        cancelled: false,
    };
    let l = Arc::new(
        ledger::state::Ledger::initialize(
            local,
            la.clone(),
            ledger::persistence::Memory::default(),
            Default::default(),
            &lc,
            1000,
            100,
        )
        .unwrap(),
    );
    l.grant(&lc, requester, 100).unwrap();
    l.set_trust(&lc, requester, ledger::accounting::Trust::new(1).unwrap())
        .unwrap();
    let lg = la
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .unwrap();
    let adapter = Arc::new(accounting::Ledger::new(l.clone(), lg, Default::default()));
    let (_, _, _, r) = selected(false);
    let runtime = Coordinator::open(
        Config {
            local,
            authority: a.clone(),
            limits: Limits::default(),
        },
        &context(&init),
        Memory::default(),
        adapter,
        r.clone(),
        true,
    )
    .unwrap();
    let key = runtime
        .admit(
            &context(&coordinate),
            accepted(a.clone(), &r, 1, 1),
            Payment::default(),
        )
        .unwrap();
    assert_eq!(
        runtime.status(&context(&observe), key),
        Ok(Some(Phase::Reserved))
    );
    assert_eq!(
        runtime.admit(
            &context(&coordinate),
            accepted(a.clone(), &r, 1, 1),
            Payment::default()
        ),
        Ok(key)
    );
    assert_eq!(
        runtime.admit(
            &context(&coordinate),
            accepted(a.clone(), &r, 1, 2),
            Payment::default()
        ),
        Err(Error::Conflict)
    );
    assert_eq!(
        runtime.supply_input(&context(&coordinate), key, Value::String("changed".into())),
        Err(Error::Invalid)
    );
    assert_eq!(
        runtime.supply_input(&context(&coordinate), key, Value::String("input".into())),
        Ok(())
    );
    assert_eq!(
        runtime.supply_input(&context(&coordinate), key, Value::String("input".into())),
        Ok(())
    );
    assert_eq!(
        runtime.supply_input(&context(&coordinate), key, Value::String("changed".into())),
        Err(Error::Conflict)
    );
    let (_, _, _, foreign) = selected(false);
    assert!(matches!(
        runtime.admit(
            &context(&coordinate),
            accepted(a.clone(), &foreign, 2, 1),
            Payment::default()
        ),
        Err(Error::Denied)
    ));
    let read = la
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    let rc = ledger::contract::Context { grant: &read, ..lc };
    assert_eq!(l.available(&rc, requester, false), Ok(93));
    assert_eq!(l.view(&rc, requester).unwrap().issued, 100);
    assert_eq!(runtime.reserve(&context(&coordinate), key), Ok(()));
    assert!(matches!(
        runtime.admit(
            &context(&observe),
            accepted(a, &r, 2, 1),
            Payment::default()
        ),
        Err(Error::Denied)
    ));
}
