//! Explicit requester consent, direct metadata and once-only commitment scenarios.
use super::*;
// A real direct metadata cache precedes quote consent, detached input/result transfer and exact two-sided accounting.
#[test]
fn explicit_request_factory_completes_metadata_then_detached_service_work() {
    use requests::{Ceiling, Invocation, Plan, Reply};
    let pair = Pair::new(true);
    let provider = endpoint(&pair);
    let requester = endpoint_world(&pair.requester);
    let cache = Arc::new(
        cache::PeerCache::new(pair.requester.registry.clone(), Default::default()).unwrap(),
    );
    assert!(matches!(
        call(&requester, &provider, 1, Plan::Board(cache.clone())).result,
        Ok(Reply::Metadata(_))
    ));
    let o = pair.provider.family.offering(pair.provider.local);
    assert!(
        cache
            .selection(pair.provider.local, &o.id, &o.service, o.version)
            .is_err()
    );
    assert!(matches!(
        call(
            &requester,
            &provider,
            3,
            Plan::Definition {
                cache: cache.clone(),
                offering: o.id.clone()
            }
        )
        .result,
        Ok(Reply::Metadata(_))
    ));
    let selection = cache
        .selection(pair.provider.local, &o.id, &o.service, o.version)
        .unwrap();
    let reply = call(
        &requester,
        &provider,
        5,
        Plan::Invoke(Box::new(Invocation {
            selection,
            cache,
            input: Value::Bytes(vec![9; 9000]),
            spending: peer::requester::Spending {
                maximum: 7,
                payment: Default::default(),
            },
            consent: Arc::new(Ceiling(7)),
        })),
    );
    let Ok(Reply::Work(key)) = reply.result else {
        panic!("completed committed work")
    };
    assert_eq!(pair.owned_available(), 93);
    assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 1);
    let r = &pair.requester;
    let observe = r
        .services
        .grant(r.local, services::authority::Action::Observe, 16_777_216)
        .unwrap();
    assert_eq!(
        r.runtime
            .completed(&service_context(&observe), key)
            .unwrap()
            .1
            .price,
        7
    );
    assert!(reply.failure.is_none());
}
// Independent control observations do not mutate owned credits or install metadata/service handlers.
#[test]
fn explicit_balance_request_finishes_only_after_normal_control_completion() {
    let pair = Pair::new(false);
    let provider = endpoint(&pair);
    let requester = endpoint_world(&pair.requester);
    let response = call(&requester, &provider, 1, requests::Plan::Balance);
    assert!(matches!(
        response.result,
        Ok(requests::Reply::Ledger(
            peer::ledger_control::query::Response::Balance(100)
        ))
    ));
    assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
    assert_eq!(pair.owned_available(), 100);
}

// A refusal of the independently validated quote cannot reserve funds, execute work or implicitly retry the consumed intent.
#[test]
fn explicit_consent_refusal_never_accepts_or_reuses_a_call() {
    use requests::{Ceiling, Invocation, Plan};
    let pair = Pair::new(false);
    let provider = endpoint(&pair);
    let requester = endpoint_world(&pair.requester);
    let cache = Arc::new(
        cache::PeerCache::new(pair.requester.registry.clone(), Default::default()).unwrap(),
    );
    call(&requester, &provider, 1, Plan::Board(cache.clone()))
        .result
        .unwrap();
    let o = pair.provider.family.offering(pair.provider.local);
    call(
        &requester,
        &provider,
        3,
        Plan::Definition {
            cache: cache.clone(),
            offering: o.id.clone(),
        },
    )
    .result
    .unwrap();
    let selection = cache
        .selection(pair.provider.local, &o.id, &o.service, o.version)
        .unwrap();
    let (plan, completed) = requester.request(Plan::Invoke(Box::new(Invocation {
        selection,
        cache,
        input: Value::String("x".into()),
        spending: peer::requester::Spending {
            maximum: 7,
            payment: Default::default(),
        },
        consent: Arc::new(Ceiling(6)),
    })));
    let client_scope = Scope {
        local: pair.requester.local,
        peer: pair.provider.local,
        connection: [19; 32],
        stream: 5,
        deadline: Instant::now() + Duration::from_secs(60),
    };
    let server_scope = Scope {
        local: client_scope.peer,
        peer: client_scope.local,
        ..client_scope
    };
    let mut outgoing = output(2_000_000);
    let mut incoming = output(2_000_000);
    let mut client = plan
        .open(client_scope, Direction::Local, &mut outgoing)
        .unwrap();
    assert!(matches!(
        plan.open(
            Scope {
                stream: 7,
                ..client_scope
            },
            Direction::Local,
            &mut output(2_000_000)
        ),
        Err(Error::Conflict)
    ));
    let mut server = provider
        .open(server_scope, Direction::Remote, &mut incoming)
        .unwrap();
    for record in drain(&mut outgoing) {
        server.record(record, &mut incoming).unwrap();
    }
    for record in drain(&mut incoming) {
        assert_eq!(
            client.record(record, &mut outgoing),
            Err(Error::Insufficient)
        );
    }
    assert!(drain(&mut outgoing).is_empty());
    let outcome = StreamOutcome::Cancelled {
        origin: crate::modules::transport::api::FailureOrigin::Local,
        reason: crate::modules::transport::api::CancellationReason::Cancelled,
    };
    client.closed(outcome).unwrap();
    server.closed(outcome).unwrap();
    let reply = completed.try_recv().unwrap();
    assert!(matches!(reply.result, Err(Error::Unknown)));
    assert_eq!(reply.cause, Some(Error::Insufficient));
    assert_eq!(pair.owned_available(), 100);
    assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
}
// An absent protected capability preserves the exact completed failure while producing no claim or balance mutation.
#[test]
fn explicit_claim_request_preserves_unsupported_remote_failure() {
    let pair = Pair::new(false);
    let provider = endpoint(&pair);
    let requester = endpoint_world(&pair.requester);
    let reply = call(
        &requester,
        &provider,
        1,
        requests::Plan::Claim {
            envelope: vec![0; ledger::redemption::ENVELOPE_BYTES],
            id: [7; 32],
        },
    );
    assert!(matches!(reply.result, Err(Error::Unsupported)));
    assert_eq!(
        reply.failure.unwrap().code,
        peer::failure::Code::Unsupported
    );
    assert_eq!(pair.owned_available(), 100);
}

// Complete explicit check calls preserve challenge-only wire semantics and commit paid/included requester outcomes once.
#[test]
fn explicit_check_factory_completes_paid_and_included_evidence() {
    use requests::{Ceiling, Challenge, Plan, Reply};
    for included in [false, true] {
        let pair = Pair::checks(included);
        let provider = endpoint(&pair);
        let requester = endpoint_world(&pair.requester);
        let cache = Arc::new(
            cache::PeerCache::new(pair.requester.registry.clone(), Default::default()).unwrap(),
        );
        call(&requester, &provider, 1, Plan::Board(cache.clone()))
            .result
            .unwrap();
        let o = pair.provider.family.offering(pair.provider.local);
        call(
            &requester,
            &provider,
            3,
            Plan::Definition {
                cache: cache.clone(),
                offering: o.id.clone(),
            },
        )
        .result
        .unwrap();
        let selection = cache
            .selection(pair.provider.local, &o.id, &o.service, o.version)
            .unwrap();
        let maximum = if included { 0 } else { 3 };
        let reply = call(
            &requester,
            &provider,
            5,
            Plan::Check(Box::new(Challenge {
                selection,
                cache,
                challenge: Value::Uint64(0),
                spending: peer::requester::Spending {
                    maximum,
                    payment: Default::default(),
                },
                consent: Arc::new(Ceiling(maximum)),
            })),
        );
        let Ok(Reply::Check(key)) = reply.result else {
            panic!("committed original check")
        };
        let r = &pair.requester;
        let grant = r
            .services
            .grant(r.local, services::authority::Action::Observe, 16_777_216)
            .unwrap();
        let rc = service_context(&grant);
        assert_eq!(
            r.runtime
                .check_outcome(&rc, key, Some(&Value::Bytes(vec![b'i']))),
            Ok(services::handler::Check::Pass)
        );
        assert_eq!(
            pair.provider.family.evidence_calls.load(Ordering::SeqCst),
            1
        );
        assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
        assert_eq!(pair.owned_available(), if included { 100 } else { 97 });
        if included {
            assert_eq!(
                r.runtime.check_allowance(&rc, key),
                Ok(services::checks::Allowance::Consumed)
            );
        }
    }
}
