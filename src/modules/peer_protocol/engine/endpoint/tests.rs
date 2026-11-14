//! Real module composition under test-owned authenticated scopes; network authentication is tested separately.
use super::*;
use crate::modules::transport::api::StreamOutcome;
use peer::{
    engine::bridge::test_support::{drain, ended, output},
    test_support::{
        invocation::Pair,
        world::{context, service_context},
    },
};
use services::value::Value;
use std::sync::atomic::Ordering;
// Bind only real independently owned module ports; custom Echo remains an explicitly installed test family.
fn endpoint(pair: &Pair) -> Endpoint<services::persistence::Memory, ledger::persistence::Memory> {
    endpoint_world(&pair.provider)
}
// Configure either independently owned endpoint without sharing its financial maps or authority issuers.
fn endpoint_world(
    p: &peer::test_support::world::World,
) -> Endpoint<services::persistence::Memory, ledger::persistence::Memory> {
    Endpoint::new(Config {
        local: p.local,
        authority: p.authority.clone(),
        services: p.services.clone(),
        board: p.board.clone(),
        board_authority: p.board_authority.clone(),
        registry: p.registry.clone(),
        runtime: p.runtime.clone(),
        ledger: p.ledger.clone(),
        ledger_authority: p.ledger_authority.clone(),
        gate: p.gate.clone(),
        transfer_policy: Arc::new(peer::ledger_control::transfer::Fixed(0)),
        claims: None,
        limits: Limits::default(),
        service_limits: Default::default(),
    })
    .unwrap()
}
// Recreate a trusted unit scope with the same original operation key, without deriving authority from wire data.
fn scope(a: Association) -> Scope {
    Scope {
        local: a.local,
        peer: a.peer,
        connection: a.operation.connection,
        stream: a.operation.stream,
        deadline: Instant::now() + Duration::from_secs(60),
    }
}
// Consume one sender record at a time without changing actual source ownership or input representation.
fn records(sender: &mut peer::input_transfer::InputSender) -> Vec<Record> {
    let mut queue = peer::outbound::Outbound::new(2_000_000).unwrap();
    while sender.enqueue_next(&mut queue, 65536).unwrap() {}
    let mut result = Vec::new();
    while let Some(bytes) = queue.front(usize::MAX) {
        let length = bytes.len();
        let mut decoder = peer::record::Decoder::default();
        let (count, record) = decoder.push(bytes).unwrap();
        assert_eq!(count, length);
        result.push(record.unwrap());
        queue.advance(count).unwrap();
    }
    result
}
// Actual quote/reservation/input/result and participant commits run once for inline and detached values.
#[test]
fn composed_provider_executes_only_complete_input_and_commits_before_results() {
    for bytes in [false, true] {
        let pair = Pair::new(bytes);
        let endpoint = endpoint(&pair);
        let input = if bytes {
            Value::Bytes(vec![9; 9000])
        } else {
            Value::String("hello".into())
        };
        let (unused, request) = pair.propose(1, input.clone(), 7);
        let a = unused.association();
        let mut queue = output(2_000_000);
        let mut handler = endpoint
            .open(scope(a), Direction::Remote, &mut queue)
            .unwrap();
        handler
            .record(Record::Control(request.bytes().to_vec()), &mut queue)
            .unwrap();
        let Record::Control(quote) = drain(&mut queue).remove(0) else {
            panic!("quote control")
        };
        assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
        let outgoing = pair
            .reserve_message(a, request, &Message::parse(&quote, 65536).unwrap())
            .unwrap();
        let r = &pair.requester;
        let invoke = r
            .authority
            .grant(r.local, Action::Invoke, 16_777_216)
            .unwrap();
        let coordinate = r
            .services
            .grant(r.local, services::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let ra = outgoing.association();
        let key = outgoing.invocation().key;
        let mut receiver = peer::result_receiver::ResultReceiver::new(
            &context(&invoke),
            r.authority.clone(),
            &outgoing,
            Limits::default(),
        )
        .unwrap();
        let mut sender =
            peer::input_transfer::InputSender::new(outgoing, Limits::default()).unwrap();
        for record in records(&mut sender) {
            handler.record(record, &mut queue).unwrap();
        }
        assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
        handler.remote_end(&mut queue).unwrap();
        let mut response = drain(&mut queue);
        for _ in 0..32 {
            if ended(&queue) {
                break;
            }
            handler.writable(&mut queue).unwrap();
            response.extend(drain(&mut queue));
        }
        assert!(ended(&queue));
        assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 1);
        for record in response {
            match record {
                Record::Control(bytes) => receiver
                    .control(
                        &context(&invoke),
                        ra,
                        &Message::parse(&bytes, 65536).unwrap(),
                    )
                    .unwrap(),
                record @ Record::Body { .. } => {
                    receiver.body(&context(&invoke), ra, &record).unwrap()
                }
            }
        }
        handler.closed(StreamOutcome::Completed).unwrap();
        receiver
            .finish(
                &context(&invoke),
                ra,
                true,
                &service_context(&coordinate),
                r.runtime.as_ref(),
            )
            .unwrap();
        let cc = service_context(&coordinate);
        r.runtime.prepare(&cc, key).unwrap();
        r.runtime
            .decide(&cc, key, services::runtime::Decision::Commit)
            .unwrap();
        r.runtime.commit(&cc, key).unwrap();
        assert_eq!(pair.owned_available(), 93);
    }
}
// Metadata and independent Ledger controls use their actual owners and do not execute a service family.
#[test]
fn metadata_controls_and_unsupported_capabilities_are_explicit() {
    let pair = Pair::new(false);
    let endpoint = endpoint(&pair);
    let (a, _, _) = pair.provider.proposal(1, &Value::String("x".into()));
    for raw in [
        br#"{"type":"board.get"}"#.as_slice(),
        br#"{"type":"ledger.balance"}"#,
    ] {
        let mut queue = output(2_000_000);
        let mut handler = endpoint
            .open(scope(a), Direction::Remote, &mut queue)
            .unwrap();
        handler
            .record(Record::Control(raw.to_vec()), &mut queue)
            .unwrap();
        handler.remote_end(&mut queue).unwrap();
        assert!(ended(&queue));
        assert_eq!(drain(&mut queue).len(), 1);
        handler.closed(StreamOutcome::Completed).unwrap();
    }
    assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
    let mut queue = output(2_000_000);
    assert!(matches!(
        endpoint.open(scope(a), Direction::Local, &mut queue),
        Err(Error::Denied)
    ));
}

// Paid/included check responses execute and commit real evidence; only complete included delivery consumes allowance.
#[test]
fn composed_checks_commit_evidence_and_reconcile_included_delivery() {
    for included in [false, true] {
        let f = peer::test_support::checks::Checks::new(included);
        let endpoint = endpoint(&f.pair);
        let (unused, request) = f.quote(1, Value::Uint64(0), 3).unwrap();
        let a = unused.association();
        let mut queue = output(2_000_000);
        let mut handler = endpoint
            .open(scope(a), Direction::Remote, &mut queue)
            .unwrap();
        handler
            .record(Record::Control(request.bytes().to_vec()), &mut queue)
            .unwrap();
        let Record::Control(bytes) = drain(&mut queue).remove(0) else {
            panic!("quote control")
        };
        let outgoing = f
            .reserve_message(a, request, &Message::parse(&bytes, 65536).unwrap())
            .unwrap();
        handler
            .record(Record::Control(outgoing.accept().to_vec()), &mut queue)
            .unwrap();
        handler
            .record(
                Record::Control(br#"{"type":"input.end"}"#.to_vec()),
                &mut queue,
            )
            .unwrap();
        assert_eq!(
            f.pair.provider.family.evidence_calls.load(Ordering::SeqCst),
            0
        );
        handler.remote_end(&mut queue).unwrap();
        let mut records = drain(&mut queue);
        for _ in 0..16 {
            if ended(&queue) {
                break;
            }
            handler.writable(&mut queue).unwrap();
            records.extend(drain(&mut queue));
        }
        assert!(ended(&queue));
        assert_eq!(
            f.pair.provider.family.evidence_calls.load(Ordering::SeqCst),
            1
        );
        assert!(records.iter().any(|r|matches!(r,Record::Control(bytes)if Message::parse(bytes,65536).unwrap().kind()==Kind::ResultEnd)));
        handler.closed(StreamOutcome::Completed).unwrap();
        if included {
            let p = &f.pair.provider;
            let grant = p
                .services
                .grant(p.local, services::authority::Action::Observe, 16_777_216)
                .unwrap();
            assert_eq!(
                p.runtime
                    .check_allowance(&service_context(&grant), peer::acceptance::peer(a).key),
                Ok(services::checks::Allowance::Consumed)
            );
        } else {
            assert_eq!(f.pair.provider.available(), 97);
        }
    }
}

// Unknown services and absent current offerings retain distinct protocol failure codes without any family execution.
#[test]
fn unsupported_service_and_missing_offering_keep_their_refusal_codes() {
    for (field, value, code) in [
        (
            "service",
            "org.example.absent",
            peer::failure::Code::Unsupported,
        ),
        ("offeringId", "absent", peer::failure::Code::NotAvailable),
    ] {
        let pair = Pair::new(false);
        let endpoint = endpoint(&pair);
        let (a, _, proposal) = pair.provider.proposal(1, &Value::String("x".into()));
        let mut wire = proposal.value().clone();
        wire[field] = serde_json::json!(value);
        let mut queue = output(2_000_000);
        let mut handler = endpoint
            .open(scope(a), Direction::Remote, &mut queue)
            .unwrap();
        handler
            .record(Record::Control(wire.to_string().into_bytes()), &mut queue)
            .unwrap();
        let Record::Control(bytes) = drain(&mut queue).remove(0) else {
            panic!("failure control")
        };
        assert_eq!(
            peer::failure::Failure::parse(&Message::parse(&bytes, 65536).unwrap())
                .unwrap()
                .code,
            code
        );
        handler.remote_end(&mut queue).unwrap();
        handler.closed(StreamOutcome::Completed).unwrap();
        assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
    }
}

// Signed transfer negotiation commits exactly once, while a matching authenticated retry returns the stored result directly.
#[test]
fn composed_transfer_negotiation_and_retained_retry_use_real_ledger() {
    use crate::modules::cryptography::{
        credential,
        primitives::{keys::signing_key, signature},
    };
    use std::collections::VecDeque;
    let pair = Pair::new(false);
    let endpoint = endpoint(&pair);
    let p = &pair.provider;
    let keys = signing_key(&mut peer::test_support::ledger::Bytes(VecDeque::from([
        vec![1; 32],
    ])))
    .unwrap();
    let owner = IdentityId(credential::identity_id(&keys.public_key).unwrap());
    let admin = p
        .ledger_authority
        .grant(p.local, ledger::authority::Action::Administer, 16_777_216)
        .unwrap();
    p.ledger
        .grant(
            &peer::test_support::world::ledger_context(&admin),
            owner,
            100,
        )
        .unwrap();
    let order = ledger::transfer::Order {
        issuer: p.local,
        owner,
        new_owner: IdentityId([4; 32]),
        transfer_id: [7; 16],
        amount: 9,
    };
    let bytes = order.encode().unwrap();
    let signature = signature::sign(&keys.private_key, &bytes, 121).unwrap();
    let wire=serde_json::json!({"type":"ledger.transfer","order":peer::base64::encode(&bytes),"ownerPublicKey":peer::base64::encode(&keys.public_key),"signature":peer::base64::encode(&signature),"maximumFee":"2"}).to_string().into_bytes();
    for stream in [1, 3] {
        let scope = Scope {
            local: p.local,
            peer: owner,
            connection: [8; 32],
            stream,
            deadline: Instant::now() + Duration::from_secs(60),
        };
        let mut queue = output(2_000_000);
        let mut handler = endpoint.open(scope, Direction::Remote, &mut queue).unwrap();
        handler
            .record(Record::Control(wire.clone()), &mut queue)
            .unwrap();
        let Record::Control(bytes) = drain(&mut queue).remove(0) else {
            panic!("transfer control")
        };
        let response = Message::parse(&bytes, 65536).unwrap();
        if stream == 1 {
            assert_eq!(response.kind(), Kind::LedgerTransferQuote);
            let accept=serde_json::json!({"type":"ledger.transfer.accept","transferId":peer::base64::encode(&order.transfer_id),"fee":"0"}).to_string().into_bytes();
            handler.record(Record::Control(accept), &mut queue).unwrap();
            let Record::Control(bytes) = drain(&mut queue).remove(0) else {
                panic!("transfer result")
            };
            assert_eq!(
                Message::parse(&bytes, 65536).unwrap().kind(),
                Kind::LedgerTransferResult
            );
        } else {
            assert_eq!(response.kind(), Kind::LedgerTransferResult);
        }
        handler.remote_end(&mut queue).unwrap();
        handler.closed(StreamOutcome::Completed).unwrap();
    }
    let control = p
        .ledger_authority
        .grant(p.local, ledger::authority::Action::Control, 16_777_216)
        .unwrap();
    let lc = peer::test_support::world::ledger_context(&control);
    assert_eq!(p.ledger.balance(&lc, owner), Ok(91));
    assert_eq!(p.ledger.balance(&lc, order.new_owner), Ok(9));
    assert_eq!(p.family.executions.load(Ordering::SeqCst), 0);
}
// Missing protected key composition reports Unsupported rather than pretending that encrypted claims are handled.
#[test]
fn absent_claim_decryptor_is_an_explicit_unsupported_response() {
    let pair = Pair::new(false);
    let endpoint = endpoint(&pair);
    let (a, _, _) = pair.provider.proposal(1, &Value::String("x".into()));
    let mut queue = output(2_000_000);
    let mut handler = endpoint
        .open(scope(a), Direction::Remote, &mut queue)
        .unwrap();
    let wire=serde_json::json!({"type":"ledger.claim.redeem","envelope":peer::base64::encode(&vec![0;ledger::redemption::ENVELOPE_BYTES])}).to_string().into_bytes();
    handler.record(Record::Control(wire), &mut queue).unwrap();
    let Record::Control(bytes) = drain(&mut queue).remove(0) else {
        panic!("failure")
    };
    assert_eq!(
        peer::failure::Failure::parse(&Message::parse(&bytes, 65536).unwrap())
            .unwrap()
            .code,
        peer::failure::Code::Unsupported
    );
    handler.remote_end(&mut queue).unwrap();
    handler.closed(StreamOutcome::Completed).unwrap();
}

// Drive actual endpoint handlers under explicit unit-only normal completion observations, never claim socket authentication here.
fn call(
    requester: &Endpoint<services::persistence::Memory, ledger::persistence::Memory>,
    provider: &Endpoint<services::persistence::Memory, ledger::persistence::Memory>,
    stream: u32,
    plan: requests::Plan,
) -> requests::Completion {
    let local = requester.0.config.local;
    let peer = provider.0.config.local;
    let client_scope = Scope {
        local,
        peer,
        connection: [19; 32],
        stream,
        deadline: Instant::now() + Duration::from_secs(60),
    };
    let server_scope = Scope {
        local: peer,
        peer: local,
        ..client_scope
    };
    let (call, completed) = requester.request(plan);
    let mut outgoing = output(2_000_000);
    let mut incoming = output(2_000_000);
    let mut client = call
        .open(client_scope, Direction::Local, &mut outgoing)
        .unwrap();
    let mut server = provider
        .open(server_scope, Direction::Remote, &mut incoming)
        .unwrap();
    let (mut client_end, mut server_end) = (false, false);
    for _ in 0..128 {
        for record in drain(&mut outgoing) {
            server.record(record, &mut incoming).unwrap();
        }
        if ended(&outgoing) && !client_end {
            server.remote_end(&mut incoming).unwrap();
            client_end = true;
        }
        for record in drain(&mut incoming) {
            client.record(record, &mut outgoing).unwrap();
        }
        if ended(&incoming) && !server_end {
            client.remote_end(&mut outgoing).unwrap();
            server_end = true;
        }
        assert!(matches!(
            completed.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        if client_end && server_end {
            break;
        }
        if !ended(&outgoing) {
            client.writable(&mut outgoing).unwrap();
        }
        if !ended(&incoming) {
            server.writable(&mut incoming).unwrap();
        }
    }
    assert!(client_end && server_end);
    server.closed(StreamOutcome::Completed).unwrap();
    client.closed(StreamOutcome::Completed).unwrap();
    completed.try_recv().unwrap()
}
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
