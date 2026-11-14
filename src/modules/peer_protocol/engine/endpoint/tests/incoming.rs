//! Incoming provider and independent control composition over actual module owners.
use super::*;
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
