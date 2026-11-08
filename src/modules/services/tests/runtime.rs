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
pub(super) fn context(grant: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    }
}
// Build exact accepted consent through a real Board and trusted original-stream quote, without network claims.
pub(super) fn accepted(
    a: Arc<Authority>,
    registry: &super::registry::Registry,
    stream: u32,
    bytes: u8,
) -> Accepted {
    accepted_terms(a, registry, stream, bytes, 7, IdentityId([2; 32]))
}
// Exact issuer and total spending bounds remain caller-selected immutable quote terms.
fn accepted_terms(
    a: Arc<Authority>,
    registry: &super::registry::Registry,
    stream: u32,
    bytes: u8,
    maximum: u64,
    issuer: IdentityId,
) -> Accepted {
    let f = Arc::new(super::test_support::family::Echo::new());
    let (owner, s, ba, _) = super::test_support::selection::selected_terms(
        false,
        f.clone(),
        Some(f),
        Arc::new(Authority::new()),
        issuer,
        IdentityId([1; 32]),
    );
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
                maximum,
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
    let family = Arc::new(super::test_support::family::Echo::new());
    let (_, _, _, r) =
        super::test_support::selection::selected_with_family(false, family, a.clone());
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
    assert_eq!(runtime.start(&context(&coordinate), key), Ok(()));
    assert_eq!(
        runtime.start(&context(&coordinate), key),
        Err(Error::Unknown)
    );
    assert_eq!(runtime.stage(&context(&coordinate), key), Ok(()));
    assert_eq!(runtime.stage(&context(&coordinate), key), Ok(()));
    assert_eq!(
        runtime.status(&context(&observe), key),
        Ok(Some(Phase::Staged))
    );
    assert_eq!(
        runtime.decide(&context(&coordinate), key, super::runtime::Decision::Commit),
        Err(Error::Conflict)
    );
    assert_eq!(runtime.prepare(&context(&coordinate), key), Ok(()));
    assert_eq!(runtime.prepare(&context(&coordinate), key), Ok(()));
    assert_eq!(
        runtime.status(&context(&observe), key),
        Ok(Some(Phase::Prepared))
    );
    assert_eq!(
        runtime.decide(&context(&coordinate), key, super::runtime::Decision::Commit),
        Ok(())
    );
    assert_eq!(
        runtime.decide(&context(&coordinate), key, super::runtime::Decision::Commit),
        Ok(())
    );
    assert_eq!(
        runtime.decide(&context(&coordinate), key, super::runtime::Decision::Abort),
        Err(Error::Conflict)
    );
    assert_eq!(
        runtime.status(&context(&observe), key),
        Ok(Some(Phase::Commit))
    );
    assert!(matches!(
        runtime.result(&context(&observe), key),
        Err(Error::Unknown)
    ));
    assert!(matches!(
        runtime.admit(
            &context(&coordinate),
            accepted(a.clone(), &r, 2, 1),
            Payment::default()
        ),
        Err(Error::Unknown)
    ));
    runtime.commit(&context(&coordinate), key).unwrap();
    runtime.commit(&context(&coordinate), key).unwrap();
    assert_eq!(
        runtime.status(&context(&observe), key),
        Ok(Some(Phase::Committed))
    );
    let result = runtime.result(&context(&observe), key).unwrap();
    assert_eq!(result.output, Value::String("input".into()));
    assert_eq!(result.result.bytes(), b"input");
    assert_eq!(result.price, 7);
    assert_eq!(l.view(&rc, requester).unwrap().issued, 93);
    assert_eq!(l.available(&rc, requester, false), Ok(93));
    assert_eq!(
        runtime.verify(&context(&observe), key, None),
        Ok(super::handler::Check::Inconclusive)
    );
    assert_eq!(
        runtime.verify(
            &context(&observe),
            key,
            Some(&Value::String("input".into()))
        ),
        Ok(super::handler::Check::Pass)
    );
    assert_eq!(
        runtime.verify(
            &context(&observe),
            key,
            Some(&Value::String("wrong".into()))
        ),
        Ok(super::handler::Check::Fail)
    );
    assert_eq!(l.view(&rc, requester).unwrap().issued, 93);
    assert_eq!(l.view(&rc, requester).unwrap().trust.millionths(), 1);
    assert_eq!(
        runtime.abort(&context(&coordinate), key),
        Err(Error::Conflict)
    );
    assert!(matches!(
        runtime.admit(
            &context(&observe),
            accepted(a, &r, 2, 1),
            Payment::default()
        ),
        Err(Error::Denied)
    ));
}

// Compose real accounting and an explicitly configured test family without exposing production admin ports.
struct Fixture {
    runtime: Coordinator<Memory>,
    a: Arc<Authority>,
    r: Arc<super::registry::Registry>,
    l: Arc<ledger::state::Ledger<ledger::persistence::Memory>>,
    la: Arc<ledger::authority::Authority>,
}
// Construct an independent real-ledger runtime for a configured local test handler.
fn runtime_with_family(family: Arc<super::test_support::family::Echo>) -> Fixture {
    configured_runtime(family, false)
}
// Requester-only fixtures share the same real finite Ledger without installing a local provider.
fn configured_runtime(
    family: Arc<super::test_support::family::Echo>,
    requester_only: bool,
) -> Fixture {
    configured_policy(family, requester_only, Default::default())
}
// Optional third-party acceptance is explicit composition policy, not peer metadata.
fn configured_policy(
    family: Arc<super::test_support::family::Echo>,
    requester_only: bool,
    policy: ledger::terms::Policy,
) -> Fixture {
    let local = IdentityId([2; 32]);
    let requester = IdentityId([1; 32]);
    let a = Arc::new(Authority::new());
    let init = a.grant(local, Action::Register, 1_000_000).unwrap();
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
    let adapter = Arc::new(accounting::Ledger::new(l.clone(), lg, policy));
    let (_, _, _, r) = if requester_only {
        super::test_support::selection::selected_requester(family, a.clone())
    } else {
        super::test_support::selection::selected_with_family(false, family, a.clone())
    };
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
    Fixture {
        runtime,
        a,
        r,
        l,
        la,
    }
}
// Invalid typed output or an over-quote final price cannot become successful settlement or trigger execution again.
#[test]
fn reject_invalid_completion_without_charge() {
    use super::test_support::family::Echo;
    for (output, price) in [(Value::Bool(false), 7), (Value::String("input".into()), 8)] {
        let family = Arc::new(Echo::with_output(output, price));
        let Fixture {
            runtime,
            a,
            r,
            l,
            la,
        } = runtime_with_family(family.clone());
        let local = IdentityId([2; 32]);
        let cg = a.grant(local, Action::Coordinate, 1_000_000).unwrap();
        let c = context(&cg);
        let key = runtime
            .admit(&c, accepted(a.clone(), &r, 1, 1), Payment::default())
            .unwrap();
        runtime
            .supply_input(&c, key, Value::String("input".into()))
            .unwrap();
        runtime.start(&c, key).unwrap();
        assert_eq!(runtime.stage(&c, key), Err(Error::Invalid));
        assert_eq!(runtime.start(&c, key), Err(Error::Unknown));
        assert_eq!(
            family.executions.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        runtime
            .decide(&c, key, super::runtime::Decision::Abort)
            .unwrap();
        runtime.abort(&c, key).unwrap();
        runtime.abort(&c, key).unwrap();
        assert_eq!(
            runtime.decide(&c, key, super::runtime::Decision::Commit),
            Err(Error::Conflict)
        );
        let rg = la
            .grant(local, ledger::authority::Action::Read, 1_000_000)
            .unwrap();
        let rc = ledger::contract::Context {
            version: 1,
            grant: &rg,
            deadline: c.deadline,
            cancelled: false,
        };
        assert_eq!(l.view(&rc, IdentityId([1; 32])).unwrap().issued, 100);
    }
}

// Cancellation does not release a hold or decide rollback; explicit authorized abort is a separate durable transition.
#[test]
fn cancellation_and_absent_hold_abort() {
    let Fixture {
        runtime,
        a,
        r,
        l,
        la,
    } = runtime_with_family(Arc::new(super::test_support::family::Echo::new()));
    let local = IdentityId([2; 32]);
    let g = a.grant(local, Action::Coordinate, 1_000_000).unwrap();
    let c = context(&g);
    let key = runtime
        .admit(&c, accepted(a.clone(), &r, 1, 1), Payment::default())
        .unwrap();
    let cancelled = Context {
        cancelled: true,
        ..c
    };
    assert_eq!(
        runtime.decide(&cancelled, key, super::runtime::Decision::Abort),
        Err(Error::Cancelled)
    );
    runtime
        .decide(&c, key, super::runtime::Decision::Abort)
        .unwrap();
    runtime.abort(&c, key).unwrap();
    let read = la
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    let rc = ledger::contract::Context {
        version: 1,
        grant: &read,
        deadline: c.deadline,
        cancelled: false,
    };
    assert_eq!(l.available(&rc, IdentityId([1; 32]), false), Ok(100));
    for stream in 2..=15 {
        runtime
            .admit(&c, accepted(a.clone(), &r, stream, 1), Payment::default())
            .unwrap();
    }
    assert_eq!(
        runtime.admit(&c, accepted(a.clone(), &r, 16, 1), Payment::default()),
        Err(Error::Insufficient)
    );
    let absent = OperationKey { stream: 16, ..key };
    runtime
        .decide(&c, absent, super::runtime::Decision::Abort)
        .unwrap();
    runtime.abort(&c, absent).unwrap();
    assert_eq!(l.available(&rc, IdentityId([1; 32]), false), Ok(2));
}

// Remote assessment and consent are independently checked against locally authorized wanted input.
fn remote_accepted(a: Arc<Authority>, r: &super::registry::Registry) -> (Accepted, Peer) {
    let (owner, selection, ba, _) = selected(true);
    let local = IdentityId([2; 32]);
    let g = a.grant(local, Action::Request, 65536).unwrap();
    let c = context(&g);
    let peer = Peer {
        local,
        remote: IdentityId([1; 32]),
        key: OperationKey {
            requester: local,
            connection: [8; 32],
            stream: 1,
        },
    };
    let auth = || {
        authorize(
            &c,
            &a,
            local,
            r,
            selection.clone(),
            peer,
            Some(Value::String("input".into())),
        )
        .unwrap()
    };
    let assessment = |metadata| super::handler::Assessment {
        metadata,
        factors: Value::Object(vec![]),
        input_bytes: 128,
        pin: None,
    };
    assert!(matches!(
        super::assessment::remote(
            &c,
            &a,
            auth(),
            assessment(Value::String("fabricated".into())),
            Limits::default()
        ),
        Err(Error::Invalid)
    ));
    let assessed = super::assessment::remote(
        &c,
        &a,
        auth(),
        assessment(Value::String("input".into())),
        Limits::default(),
    )
    .unwrap();
    let q = Quoter::new(local, a, Limits::default()).unwrap();
    let quote = q
        .issue(
            &c,
            Issue {
                assessed,
                bytes: Blob::new(vec![1], 1).unwrap(),
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
    (
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
        .unwrap(),
        peer,
    )
}
// A wanted publisher can receive and settle remote work with a requester codec and no local provider.
#[test]
fn requester_only_wanted_result_and_uncertainty() {
    let Fixture {
        runtime,
        a,
        r,
        l,
        la,
    } = configured_runtime(Arc::new(super::test_support::family::Echo::new()), true);
    let local = IdentityId([2; 32]);
    assert!(r.select("org.example.echo", 1, true).is_err());
    let g = a.grant(local, Action::Coordinate, 1_000_000).unwrap();
    let c = context(&g);
    let (accepted, peer) = remote_accepted(a.clone(), &r);
    let key = runtime.admit(&c, accepted, Payment::default()).unwrap();
    runtime
        .supply_input(&c, key, Value::String("input".into()))
        .unwrap();
    assert_eq!(runtime.start(&c, key), Err(Error::Unsupported));
    let (invocation, input) = runtime.outbound(&c, key).unwrap();
    assert_eq!(invocation.requester, local);
    assert_eq!(invocation.provider, peer.remote);
    assert_eq!(input, Value::String("input".into()));
    assert!(matches!(runtime.outbound(&c, key), Err(Error::Unknown)));
    assert_eq!(
        runtime.decide(&c, key, super::runtime::Decision::Abort),
        Err(Error::Unknown)
    );
    assert_eq!(runtime.resume(&c, key), Err(Error::Unknown));
    let result = || super::runtime::RemoteResult {
        output: Value::String("input".into()),
        bytes: Blob::new(b"input".to_vec(), 128).unwrap(),
        price: 7,
    };
    assert_eq!(
        runtime.receive(
            &c,
            Peer {
                remote: IdentityId([9; 32]),
                ..peer
            },
            result()
        ),
        Err(Error::Denied)
    );
    runtime.receive(&c, peer, result()).unwrap();
    runtime.receive(&c, peer, result()).unwrap();
    assert_eq!(
        runtime.decide(&c, key, super::runtime::Decision::Abort),
        Err(Error::Unknown)
    );
    assert_eq!(runtime.resume(&c, key), Ok(Phase::Committed));
    assert_eq!(runtime.decline(&c, peer), Err(Error::Conflict));
    let read = la
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    let rc = ledger::contract::Context {
        version: 1,
        grant: &read,
        deadline: c.deadline,
        cancelled: false,
    };
    assert_eq!(l.view(&rc, peer.remote).unwrap().issued, 107);
    assert_eq!(l.budgets(&rc).unwrap().0, 893);
}
// An explicit original-provider uncommitted failure can release the original requester hold; a timeout cannot.
#[test]
fn requester_explicit_decline() {
    let Fixture {
        runtime,
        a,
        r,
        l,
        la,
    } = configured_runtime(Arc::new(super::test_support::family::Echo::new()), true);
    let local = IdentityId([2; 32]);
    let g = a.grant(local, Action::Coordinate, 1_000_000).unwrap();
    let c = context(&g);
    let (accepted, peer) = remote_accepted(a.clone(), &r);
    let key = runtime.admit(&c, accepted, Payment::default()).unwrap();
    runtime
        .supply_input(&c, key, Value::String("input".into()))
        .unwrap();
    runtime.outbound(&c, key).unwrap();
    runtime.decline(&c, peer).unwrap();
    runtime.abort(&c, key).unwrap();
    assert_eq!(
        runtime.decide(&c, key, super::runtime::Decision::Commit),
        Err(Error::Conflict)
    );
    let read = la
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    let rc = ledger::contract::Context {
        version: 1,
        grant: &read,
        deadline: c.deadline,
        cancelled: false,
    };
    assert_eq!(l.view(&rc, peer.remote).unwrap().issued, 100);
    assert_eq!(l.budgets(&rc).unwrap().0, 900);
}

// Successful third-party work remains committed while issuer refusal, uncertainty and authenticated receipt evolve independently.
#[test]
fn third_party_payment_is_independent() {
    let family = Arc::new(super::test_support::family::Echo::new());
    let Fixture {
        runtime,
        a,
        r,
        l,
        la,
    } = configured_policy(
        family.clone(),
        false,
        ledger::terms::Policy {
            third_party: true,
            ..Default::default()
        },
    );
    let local = IdentityId([2; 32]);
    let requester = IdentityId([1; 32]);
    let issuer = IdentityId([3; 32]);
    let g = a.grant(local, Action::Coordinate, 1_000_000).unwrap();
    let c = context(&g);
    let observe = a.grant(local, Action::Observe, 1_000_000).unwrap();
    let key = runtime
        .admit(
            &c,
            accepted_terms(a.clone(), &r, 1, 1, 9, issuer),
            Payment {
                transfer: Some([4; 16]),
                fee: Some(2),
            },
        )
        .unwrap();
    assert_eq!(
        runtime.payment(&context(&observe), key),
        Err(Error::Unknown)
    );
    runtime
        .supply_input(&c, key, Value::String("input".into()))
        .unwrap();
    runtime.start(&c, key).unwrap();
    assert_eq!(runtime.resume(&c, key), Ok(Phase::Committed));
    assert_eq!(
        runtime.payment(&context(&observe), key),
        Ok(ledger::payment::Payment::Pending)
    );
    let request = runtime.payment_request(&c, key).unwrap();
    assert!(!request.submit);
    assert_eq!(request.order.amount, 9);
    assert_eq!(request.fee, 2);
    runtime.payment_problem(&c, key, false).unwrap();
    assert_eq!(
        runtime.payment(&context(&observe), key),
        Ok(ledger::payment::Payment::Unsettled)
    );
    runtime.payment_problem(&c, key, true).unwrap();
    assert_eq!(
        runtime.payment(&context(&observe), key),
        Ok(ledger::payment::Payment::Unknown)
    );
    assert_eq!(runtime.result(&context(&observe), key).unwrap().price, 7);
    let evidence = la
        .grant(local, ledger::authority::Action::Evidence, 1_000_000)
        .unwrap();
    let ec = ledger::contract::Context {
        version: 1,
        grant: &evidence,
        deadline: c.deadline,
        cancelled: false,
    };
    let receipt = |amount| {
        ledger::payment::Receipt::authenticated(
            &ec,
            la.clone(),
            local,
            issuer,
            ledger::transfer_state::TransferResult {
                order: ledger::transfer::Order {
                    transfer_id: [4; 16],
                    issuer,
                    owner: requester,
                    new_owner: local,
                    amount,
                },
                fee: 2,
            },
        )
        .unwrap()
    };
    assert_eq!(
        runtime.confirm_payment(&c, key, &receipt(10)),
        Err(Error::Denied)
    );
    runtime.confirm_payment(&c, key, &receipt(9)).unwrap();
    runtime.confirm_payment(&c, key, &receipt(9)).unwrap();
    assert_eq!(
        runtime.payment(&context(&observe), key),
        Ok(ledger::payment::Payment::Settled)
    );
    let read = la
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    let rc = ledger::contract::Context { grant: &read, ..ec };
    assert_eq!(l.view(&rc, issuer).unwrap().owned, 7);
    assert_eq!(
        family.executions.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(
        runtime.decide(&c, key, super::runtime::Decision::Abort),
        Err(Error::Conflict)
    );
}

// Draining excludes new work but cannot discard the exact code needed by an accepted reservation.
#[test]
fn accepted_work_survives_handler_drain() {
    let family = Arc::new(super::test_support::family::Echo::new());
    let Fixture {
        runtime,
        a,
        r,
        l,
        la,
    } = runtime_with_family(family.clone());
    let local = IdentityId([2; 32]);
    let coordinate = a.grant(local, Action::Coordinate, 1_000_000).unwrap();
    let c = context(&coordinate);
    let key = runtime
        .admit(&c, accepted(a.clone(), &r, 1, 1), Payment::default())
        .unwrap();
    runtime
        .supply_input(&c, key, Value::String("input".into()))
        .unwrap();
    let register = a.grant(local, Action::Register, 1_000_000).unwrap();
    r.drain(&context(&register), "org.example.echo", 1).unwrap();
    assert!(r.select("org.example.echo", 1, true).is_err());
    runtime.start(&c, key).unwrap();
    assert_eq!(runtime.resume(&c, key), Ok(Phase::Committed));
    assert_eq!(
        family.executions.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    let read = la
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    assert_eq!(
        l.view(
            &ledger::contract::Context {
                version: 1,
                grant: &read,
                deadline: c.deadline,
                cancelled: false
            },
            IdentityId([1; 32])
        )
        .unwrap()
        .issued,
        93
    );
}
