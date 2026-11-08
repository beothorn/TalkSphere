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
    configured_limits(family, requester_only, policy, Limits::default())
}
// Runtime capacity varies independently of the unchanged registry and finite accounting participant.
fn configured_limits(
    family: Arc<super::test_support::family::Echo>,
    requester_only: bool,
    policy: ledger::terms::Policy,
    limits: Limits,
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
            limits,
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

// A full lifetime operation table refuses new consent while preserving the first result and exact charge.
#[test]
fn lifetime_capacity_keeps_results() {
    let f = Arc::new(super::test_support::family::Echo::new());
    let fixture = configured_limits(
        f.clone(),
        false,
        Default::default(),
        Limits {
            operations: 1,
            bodies: 1,
            ..Limits::default()
        },
    );
    let Fixture { runtime, a, r, .. } = fixture;
    let g = a
        .grant(IdentityId([2; 32]), Action::Coordinate, 1_000_000)
        .unwrap();
    let o = a
        .grant(IdentityId([2; 32]), Action::Observe, 1_000_000)
        .unwrap();
    let accepted = accepted(a.clone(), &r, 1, 1);
    let key = accepted.assessed().authorized.peer.key;
    runtime
        .admit(&context(&g), accepted, Payment::default())
        .unwrap();
    runtime
        .supply_input(&context(&g), key, Value::String("input".into()))
        .unwrap();
    runtime.start(&context(&g), key).unwrap();
    runtime.stage(&context(&g), key).unwrap();
    runtime.resume(&context(&g), key).unwrap();
    let result = runtime.result(&context(&o), key).unwrap();
    assert_eq!(result.price, 7);
    assert_eq!(
        runtime.admit(
            &context(&g),
            self::accepted(a.clone(), &r, 2, 2),
            Payment::default()
        ),
        Err(Error::Limit)
    );
    assert_eq!(
        runtime.result(&context(&o), key).unwrap().result.bytes(),
        result.result.bytes()
    );
    assert_eq!(f.executions.load(std::sync::atomic::Ordering::SeqCst), 1);
}
// Independent coordinators produce identical visible outcomes through direct and serialized calls, even after a lost dispatch reply.
#[test]
fn serialized_and_direct_contracts_match() {
    use super::adapter::{Adapter, Request};
    use super::runtime::Decision;
    let direct = runtime_with_family(Arc::new(super::test_support::family::Echo::new()));
    let family = Arc::new(super::test_support::family::Echo::new());
    let remote = runtime_with_family(family.clone());
    let dg = direct
        .a
        .grant(IdentityId([2; 32]), Action::Coordinate, 1_000_000)
        .unwrap();
    let rg = remote
        .a
        .grant(IdentityId([2; 32]), Action::Coordinate, 1_000_000)
        .unwrap();
    let obs = remote
        .a
        .grant(IdentityId([2; 32]), Action::Observe, 1_000_000)
        .unwrap();
    let d = accepted(direct.a.clone(), &direct.r, 1, 1);
    let r = accepted(remote.a.clone(), &remote.r, 1, 1);
    assert_eq!(d.bytes().bytes(), r.bytes().bytes());
    assert_eq!(
        <sha2::Sha256 as sha2::Digest>::digest(d.bytes().bytes()),
        <sha2::Sha256 as sha2::Digest>::digest(r.bytes().bytes())
    );
    let key = d.assessed().authorized.peer.key;
    direct
        .runtime
        .admit(&context(&dg), d, Payment::default())
        .unwrap();
    remote
        .runtime
        .admit(&context(&rg), r, Payment::default())
        .unwrap();
    let adapter = Adapter::new(&remote.runtime, Limits::default()).unwrap();
    let input = Value::String("input".into());
    direct
        .runtime
        .supply_input(&context(&dg), key, input.clone())
        .unwrap();
    adapter
        .dispatch(
            &context(&rg),
            &adapter.encode(Request::Input(key, input)).unwrap(),
        )
        .unwrap();
    direct.runtime.start(&context(&dg), key).unwrap();
    let start = adapter.encode(Request::Start(key)).unwrap();
    drop(adapter.dispatch(&context(&rg), &start).unwrap());
    assert_eq!(adapter.dispatch(&context(&rg), &start), Err(Error::Unknown));
    direct.runtime.stage(&context(&dg), key).unwrap();
    direct.runtime.prepare(&context(&dg), key).unwrap();
    direct
        .runtime
        .decide(&context(&dg), key, Decision::Commit)
        .unwrap();
    direct.runtime.commit(&context(&dg), key).unwrap();
    for request in [
        Request::Stage(key),
        Request::Prepare(key),
        Request::Decide(key, Decision::Commit),
        Request::Commit(key),
    ] {
        assert_eq!(
            adapter.decode(
                &adapter
                    .dispatch(&context(&rg), &adapter.encode(request).unwrap())
                    .unwrap()
            ),
            Ok(Value::Bool(true))
        );
    }
    let result = adapter.encode(Request::Result(key)).unwrap();
    let reply = adapter.dispatch(&context(&obs), &result).unwrap();
    let dob = direct
        .a
        .grant(IdentityId([2; 32]), Action::Observe, 1_000_000)
        .unwrap();
    let expected = direct.runtime.result(&context(&dob), key).unwrap();
    assert_eq!(
        adapter.decode(&reply),
        Ok(Value::Array(vec![
            expected.output,
            Value::Bytes(expected.result.bytes().to_vec()),
            Value::Uint64(7)
        ]))
    );
    assert_eq!(adapter.dispatch(&context(&obs), &result).unwrap(), reply);
    let status = adapter.encode(Request::Status(key)).unwrap();
    assert_eq!(
        adapter.decode(&adapter.dispatch(&context(&obs), &status).unwrap()),
        Ok(Value::String("Committed".into()))
    );
    let foreign = Authority::new()
        .grant(IdentityId([2; 32]), Action::Observe, 1_000_000)
        .unwrap();
    assert_eq!(
        adapter.dispatch(&context(&foreign), &status),
        Err(Error::Denied)
    );
    for n in 0..status.len() {
        assert!(adapter.dispatch(&context(&obs), &status[..n]).is_err());
    }
    let mut future = status.clone();
    // Array header is five bytes; the first uint32 value follows its one-byte tag.
    future[9] = 2;
    assert_eq!(
        adapter.dispatch(&context(&obs), &future),
        Err(Error::Unsupported)
    );
    let narrow = Adapter::new(
        &remote.runtime,
        Limits {
            state_bytes: 8,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(narrow.dispatch(&context(&obs), &status), Err(Error::Limit));
    assert_eq!(
        family.executions.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    let second = accepted(remote.a.clone(), &remote.r, 2, 2);
    let other_key = second.assessed().authorized.peer.key;
    remote
        .runtime
        .admit(&context(&rg), second, Payment::default())
        .unwrap();
    for request in [
        Request::Decide(other_key, Decision::Abort),
        Request::Abort(other_key),
    ] {
        adapter
            .dispatch(&context(&rg), &adapter.encode(request).unwrap())
            .unwrap();
    }
    assert_eq!(
        remote.runtime.status(&context(&obs), other_key),
        Ok(Some(Phase::Aborted))
    );
    // The original reply remains valid after the entire adapter is gone.
    drop(remote.runtime);
    assert!(!reply.is_empty());
}
// Simultaneous dispatch and decision delivery serialize to one execution and one immutable terminal outcome.
#[test]
fn concurrent_dispatch_and_decisions() {
    use super::runtime::Decision;
    use std::sync::{Barrier, atomic::Ordering};
    let f = Arc::new(super::test_support::family::Echo::new());
    let fixture = runtime_with_family(f.clone());
    let g = fixture
        .a
        .grant(IdentityId([2; 32]), Action::Coordinate, 1_000_000)
        .unwrap();
    let o = fixture
        .a
        .grant(IdentityId([2; 32]), Action::Observe, 1_000_000)
        .unwrap();
    let accepted = accepted(fixture.a.clone(), &fixture.r, 1, 1);
    let key = accepted.assessed().authorized.peer.key;
    fixture
        .runtime
        .admit(&context(&g), accepted, Payment::default())
        .unwrap();
    fixture
        .runtime
        .supply_input(&context(&g), key, Value::String("input".into()))
        .unwrap();
    let gate = Barrier::new(3);
    std::thread::scope(|scope| {
        let one = scope.spawn(|| {
            gate.wait();
            fixture.runtime.start(&context(&g), key)
        });
        let two = scope.spawn(|| {
            gate.wait();
            fixture.runtime.start(&context(&g), key)
        });
        gate.wait();
        let results = [one.join().unwrap(), two.join().unwrap()];
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(results.contains(&Err(Error::Unknown)));
    });
    assert_eq!(f.executions.load(Ordering::SeqCst), 1);
    fixture.runtime.stage(&context(&g), key).unwrap();
    fixture.runtime.prepare(&context(&g), key).unwrap();
    let gate = Barrier::new(3);
    let decision = std::thread::scope(|scope| {
        let one = scope.spawn(|| {
            gate.wait();
            fixture.runtime.decide(&context(&g), key, Decision::Commit)
        });
        let two = scope.spawn(|| {
            gate.wait();
            fixture.runtime.decide(&context(&g), key, Decision::Abort)
        });
        gate.wait();
        let a = one.join().unwrap();
        let b = two.join().unwrap();
        assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
        assert!(a == Err(Error::Conflict) || b == Err(Error::Conflict));
        if a.is_ok() {
            Decision::Commit
        } else {
            Decision::Abort
        }
    });
    let gate = Barrier::new(3);
    std::thread::scope(|scope| {
        let deliver = || {
            gate.wait();
            match decision {
                Decision::Commit => fixture.runtime.commit(&context(&g), key),
                Decision::Abort => fixture.runtime.abort(&context(&g), key),
            }
        };
        let one = scope.spawn(deliver);
        let two = scope.spawn(deliver);
        gate.wait();
        assert_eq!(one.join().unwrap(), Ok(()));
        assert_eq!(two.join().unwrap(), Ok(()));
    });
    let terminal = fixture.runtime.status(&context(&o), key).unwrap();
    assert_eq!(
        terminal,
        Some(if decision == Decision::Commit {
            Phase::Committed
        } else {
            Phase::Aborted
        })
    );
    fixture.a.revoke();
    assert_eq!(
        fixture.runtime.status(&context(&o), key),
        Err(Error::Denied)
    );
    assert_eq!(f.executions.load(Ordering::SeqCst), 1);
}
// Literal upstream Computation records settle actual owned credits, releasing unused maximum without installing a runtime.
#[test]
fn metered_and_negative_completions_settle_real_ledger() {
    use super::handler::Requester;
    use super::runtime::RemoteResult;
    use super::test_support::computation::{Catalogue, Contract};
    use std::collections::BTreeMap;
    for (case, outcome, billed, price, maximum) in [
        ("computation", "result", 1201, 2, 60),
        ("computation", "executionError", 1201, 2, 60),
        ("computation", "timeLimitReached", 60000, 60, 60),
        ("discovery", "empty", 0, 2, 2),
        ("feeds", "empty", 0, 1, 1),
    ] {
        let fixture = runtime_with_family(Arc::new(super::test_support::family::Echo::new()));
        let local = IdentityId([2; 32]);
        let remote = IdentityId([1; 32]);
        let a = fixture.a;
        let init = a.grant(local, Action::Register, 1_000_000).unwrap();
        let cg = a.grant(local, Action::Coordinate, 1_000_000).unwrap();
        let rg = a.grant(local, Action::Request, 1_000_000).unwrap();
        let og = a.grant(local, Action::Observe, 1_000_000).unwrap();
        let registry = Arc::new(
            super::registry::Registry::new(
                local,
                a.clone(),
                Limits::default(),
                Arc::new(Catalogue),
            )
            .unwrap(),
        );
        let definition: &[u8] = match case {
            "discovery" => include_bytes!("../test_support/discovery-definition.json"),
            "feeds" => include_bytes!("../test_support/feeds-definition.json"),
            _ => include_bytes!("../test_support/computation-definition.json"),
        };
        let mut source = serde_json::json!({"id":"computation-standard","service":"cr2se.computation","serviceVersion":1,"description":"Evaluate requester-supplied source code for a bounded time.","creditIssuer":crate::modules::identity::text::format(remote),"pricing":{"model":"cr2se.computation.v1","minimumPrice":1,"executionTimeFactor":{"numerator":1,"denominator":1000}},"info":{"minimumExecutionMilliseconds":1,"maximumExecutionMilliseconds":60000,"maximumSourceBytes":1000000,"runtimes":[{"language":"javascript","languageVersion":"ECMAScript 2024"}]}});
        if case != "computation" {
            source = serde_json::from_str(if case == "discovery" {
                include_str!("../test_support/discovery-offering.json")
            } else {
                include_str!("../test_support/feeds-offering.json")
            })
            .unwrap();
            source["creditIssuer"] = crate::modules::identity::text::format(remote).into();
        }
        let raw = serde_json::json!({"version":1,"providedServices":[],"wantedServices":[source]})
            .to_string();
        let board = board::offering::Board::parse(raw.as_bytes(), Default::default()).unwrap();
        let o = board.offerings.values().next().unwrap();
        let family = Arc::new(Contract(
            board::definition::Definition::parse(definition, o, Default::default()).unwrap(),
        ));
        registry
            .register(&context(&init), family.clone(), None)
            .unwrap();
        let ba = Arc::new(board::authority::Authority::new());
        let owner =
            board::state::Provider::new(local, ba.clone(), Default::default(), registry.clone())
                .unwrap();
        let pubg = ba
            .grant(local, board::authority::Action::Publish, 1_000_000)
            .unwrap();
        let bg = ba
            .grant(local, board::authority::Action::Select, 1_000_000)
            .unwrap();
        let bc = board::contract::Context {
            grant: &pubg,
            deadline: context(&cg).deadline,
            cancelled: false,
        };
        let prepared = board::prepared::Prepared::new(
            local,
            raw.as_bytes(),
            &BTreeMap::from([(o.id.clone(), family.definition().document.bytes().to_vec())]),
            Default::default(),
            registry.as_ref(),
        )
        .unwrap();
        let snapshot = owner.publish(&bc, prepared).unwrap();
        let bc = board::contract::Context { grant: &bg, ..bc };
        let selection = owner
            .select(
                &bc,
                &snapshot,
                &board::selection::Request {
                    id: &o.id,
                    service: &o.service,
                    version: 1,
                    description: &o.description,
                    other: remote,
                    authenticated: true,
                },
            )
            .unwrap();
        let mut input = Value::Object(vec![
            ("language".into(), Value::String("javascript".into())),
            (
                "languageVersion".into(),
                Value::String("ECMAScript 2024".into()),
            ),
            ("source".into(), Value::String("1+1".into())),
            ("maxExecutionMilliseconds".into(), Value::Uint64(60000)),
        ]);
        if case != "computation" {
            let mut fields = vec![
                (
                    "operation".into(),
                    Value::String(
                        if case == "discovery" {
                            "listIdentities"
                        } else {
                            "read"
                        }
                        .into(),
                    ),
                ),
                ("limit".into(), Value::Uint32(1)),
            ];
            if case == "feeds" {
                fields.push(("selector".into(), Value::String("latest".into())));
            }
            input = Value::Object(fields);
        }
        let peer = Peer {
            local,
            remote,
            key: OperationKey {
                requester: local,
                connection: [18; 32],
                stream: 1,
            },
        };
        let auth = authorize(
            &context(&rg),
            &a,
            local,
            &registry,
            selection,
            peer,
            Some(input.clone()),
        )
        .unwrap();
        let assessed = super::assessment::remote(
            &context(&rg),
            &a,
            auth,
            super::handler::Assessment {
                metadata: input.clone(),
                factors: Value::Object(vec![]),
                input_bytes: 4096,
                pin: None,
            },
            Limits::default(),
        )
        .unwrap();
        let quoter = Quoter::new(local, a.clone(), Limits::default()).unwrap();
        let quote = quoter
            .issue(
                &context(&rg),
                Issue {
                    assessed,
                    bytes: Blob::new(b"exact-local-metered-quote".to_vec(), 128).unwrap(),
                    maximum,
                    expires: bc.deadline,
                },
            )
            .unwrap();
        let accepted = quoter
            .accept(
                &context(&rg),
                &quote,
                peer,
                quote.hash(),
                &BoardCurrent {
                    owner: &owner,
                    context: &bc,
                },
            )
            .unwrap();
        let admin = fixture
            .la
            .grant(local, ledger::authority::Action::Administer, 1_000_000)
            .unwrap();
        let lc = ledger::contract::Context {
            version: 1,
            grant: &admin,
            deadline: bc.deadline,
            cancelled: false,
        };
        fixture.l.set_owned(&lc, remote, 100).unwrap();
        let lg = fixture
            .la
            .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
            .unwrap();
        let accounting = Arc::new(accounting::Ledger::new(
            fixture.l.clone(),
            lg,
            Default::default(),
        ));
        let runtime = Coordinator::open(
            Config {
                local,
                authority: a,
                limits: Limits::default(),
            },
            &context(&init),
            Memory::default(),
            accounting,
            registry,
            true,
        )
        .unwrap();
        let key = runtime
            .admit(&context(&cg), accepted, Payment::default())
            .unwrap();
        let read = fixture
            .la
            .grant(local, ledger::authority::Action::Read, 1_000_000)
            .unwrap();
        let readc = ledger::contract::Context { grant: &read, ..lc };
        assert_eq!(fixture.l.available(&readc, remote, true), Ok(100 - maximum));
        runtime.supply_input(&context(&cg), key, input).unwrap();
        let _ = runtime.outbound(&context(&cg), key).unwrap();
        let mut output = vec![
            ("outcome".into(), Value::String(outcome.into())),
            ("billedMilliseconds".into(), Value::Uint64(billed)),
            ("price".into(), Value::Uint64(price)),
        ];
        if outcome == "result" {
            output.push(("result".into(), Value::String("2".into())));
        }
        if case != "computation" {
            output = vec![
                (
                    "operation".into(),
                    Value::String(
                        if case == "discovery" {
                            "listIdentities"
                        } else {
                            "read"
                        }
                        .into(),
                    ),
                ),
                (
                    if case == "discovery" {
                        "identities"
                    } else {
                        "messages"
                    }
                    .into(),
                    Value::Array(vec![]),
                ),
                ("more".into(), Value::Bool(false)),
                ("price".into(), Value::Uint64(price)),
            ];
            if case == "feeds" {
                output.push(("selector".into(), Value::String("latest".into())));
            }
        }
        runtime
            .receive(
                &context(&cg),
                peer,
                RemoteResult {
                    output: Value::Object(output),
                    bytes: Blob::new(vec![1, 2], 2).unwrap(),
                    price,
                },
            )
            .unwrap();
        runtime.resume(&context(&cg), key).unwrap();
        assert_eq!(runtime.result(&context(&og), key).unwrap().price, price);
        assert_eq!(fixture.l.view(&readc, remote).unwrap().owned, 100 - price);
        assert_eq!(fixture.l.available(&readc, remote, true), Ok(100 - price));
    }
}

// Complete logical byte bodies reject missing/extra content, a different hash and wrong type without executing or charging.
#[test]
fn complete_body_validation_precedes_execution() {
    let fixture = runtime_with_family(Arc::new(super::test_support::family::Echo::new()));
    let local = IdentityId([2; 32]);
    let family = Arc::new(super::test_support::body_family::Body::new());
    let provider = Arc::new(super::test_support::family::Echo::new());
    let (owner, selection, ba, registry) = super::test_support::selection::selected_ports(
        false,
        family.clone(),
        Arc::new(super::test_support::family::Echo::new()),
        fixture.a.clone(),
    );
    let invoke = fixture.a.grant(local, Action::Invoke, 1_000_000).unwrap();
    let g = fixture
        .a
        .grant(local, Action::Coordinate, 1_000_000)
        .unwrap();
    let init = fixture.a.grant(local, Action::Register, 1_000_000).unwrap();
    let peer = Peer {
        local,
        remote: IdentityId([1; 32]),
        key: OperationKey {
            requester: IdentityId([1; 32]),
            connection: [22; 32],
            stream: 1,
        },
    };
    let auth = authorize(
        &context(&invoke),
        &fixture.a,
        local,
        &registry,
        selection,
        peer,
        None,
    )
    .unwrap();
    // This trusted provider assessment is a test-owned immutable contract result, not peer-supplied authorization.
    let assessed = super::assessment::Assessed {
        authorized: auth,
        assessment: super::handler::Assessment {
            metadata: Value::Bytes(vec![0, 1, 255]),
            factors: Value::Object(vec![]),
            input_bytes: 3,
            pin: None,
        },
    };
    let q = Quoter::new(local, fixture.a.clone(), Limits::default()).unwrap();
    let quote = q
        .issue(
            &context(&invoke),
            Issue {
                assessed,
                bytes: Blob::new(vec![9], 1).unwrap(),
                maximum: 7,
                expires: context(&g).deadline,
            },
        )
        .unwrap();
    let bg = ba
        .grant(local, board::authority::Action::Select, 1_000_000)
        .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: context(&g).deadline,
        cancelled: false,
    };
    let accepted = q
        .accept(
            &context(&invoke),
            &quote,
            peer,
            quote.hash(),
            &BoardCurrent {
                owner: &owner,
                context: &bc,
            },
        )
        .unwrap();
    let lg = fixture
        .la
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .unwrap();
    let accounting = Arc::new(accounting::Ledger::new(
        fixture.l.clone(),
        lg,
        Default::default(),
    ));
    let runtime = Coordinator::open(
        Config {
            local,
            authority: fixture.a.clone(),
            limits: Limits::default(),
        },
        &context(&init),
        Memory::default(),
        accounting,
        registry,
        true,
    )
    .unwrap();
    let key = runtime
        .admit(&context(&g), accepted, Payment::default())
        .unwrap();
    for input in [
        Value::Bytes(vec![0, 1]),
        Value::Bytes(vec![0, 1, 255, 2]),
        Value::Bytes(vec![0, 2, 255]),
        Value::Bool(false),
    ] {
        assert!(runtime.supply_input(&context(&g), key, input).is_err());
        assert_eq!(runtime.start(&context(&g), key), Err(Error::Invalid));
    }
    assert_eq!(
        provider
            .executions
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    let read = fixture
        .la
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    let lc = ledger::contract::Context {
        version: 1,
        grant: &read,
        deadline: context(&g).deadline,
        cancelled: false,
    };
    assert_eq!(fixture.l.view(&lc, peer.remote).unwrap().issued, 100);
    runtime
        .supply_input(&context(&g), key, Value::Bytes(vec![0, 1, 255]))
        .unwrap();
}
