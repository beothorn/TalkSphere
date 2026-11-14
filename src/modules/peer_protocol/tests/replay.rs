//! Narrow Messaging port fixtures exercise original cached results and once-only accounting; no production Messaging or signature backend is installed.
use super::{
    Error,
    authority::Action,
    contract::{Association, Limits},
    exchange::{Direction, Exchange, Outcome},
    message::{Kind, Message},
    outbound::Outbound,
    record::{Decoder, Record},
    replay::{self, Lookup},
    result_receiver::ResultReceiver,
    test_support::{
        invocation::Pair,
        messaging::Messaging,
        world::{World, context, service_context},
    },
};
use crate::modules::{board, services};
use std::{
    sync::{Arc, atomic::Ordering},
    time::Instant,
};
// Two independently owned real module stacks explicitly install only the narrow pinned-contract replay fixture.
fn stacks() -> Pair<Messaging> {
    Pair::from_worlds(
        World::build(false, Arc::new(Messaging::new())),
        World::build(true, Arc::new(Messaging::new())),
    )
}
// Actual incremental framing drains body/control records without relying on the result producer's serialization.
fn drain(queue: &mut Outbound) -> Vec<Record> {
    let mut decoder = Decoder::default();
    let mut records = Vec::new();
    while queue.pending() > 0 {
        let data = queue.front(13).unwrap().to_vec();
        let mut offset = 0;
        while offset < data.len() {
            let (n, r) = decoder.push(&data[offset..]).unwrap();
            offset += n;
            if let Some(r) = r {
                records.push(r);
            }
        }
        queue.advance(data.len()).unwrap();
    }
    decoder.finish().unwrap();
    records
}
// Both recovery and removal return their original complete result after session loss, even with publication removed and provider drained.
#[test]
fn cached_retry_returns_original_output_and_charges_at_most_once() {
    for remove in [false, true] {
        let pair = stacks();
        let p = &pair.provider;
        let r = &pair.requester;
        let input = Messaging::input(remove);
        let (issued, request) = pair.propose(1, input.clone(), 7);
        let original = issued.association();
        let outgoing = pair.reserve(&issued, request).unwrap();
        let incoming = pair
            .accept(
                issued,
                original,
                &Message::parse(outgoing.accept(), 65536).unwrap(),
            )
            .unwrap();
        pair.commit_provider(incoming, &input);
        let original_key = super::acceptance::peer(outgoing.association()).key;
        assert_eq!(p.family.executions(), 1);
        drop(outgoing);
        // Removing current metadata and draining selection cannot erase previously committed family replay obligations.
        let publish = p
            .board_authority
            .grant(p.local, board::authority::Action::Publish, 16_777_216)
            .unwrap();
        let bc = board::contract::Context {
            grant: &publish,
            deadline: Instant::now() + std::time::Duration::from_secs(60),
            cancelled: false,
        };
        p.board
            .publish(
                &bc,
                board::prepared::Prepared::new(
                    p.local,
                    br#"{"version":1,"providedServices":[],"wantedServices":[]}"#,
                    &Default::default(),
                    Default::default(),
                    p.registry.as_ref(),
                )
                .unwrap(),
            )
            .unwrap();
        let reg = p
            .services
            .grant(p.local, services::authority::Action::Register, 16_777_216)
            .unwrap();
        p.registry
            .drain(&service_context(&reg), "cr2se.messaging", 1)
            .unwrap();
        // Only the already reserved original requester payment remains; a fresh ordinary invocation would be unfunded.
        let pa = p
            .ledger_authority
            .grant(
                p.local,
                crate::modules::ledger::authority::Action::Administer,
                16_777_216,
            )
            .unwrap();
        p.ledger
            .revoke(
                &super::test_support::world::ledger_context(&pa),
                r.local,
                93,
            )
            .unwrap();
        let ra = r
            .ledger_authority
            .grant(
                r.local,
                crate::modules::ledger::authority::Action::Administer,
                16_777_216,
            )
            .unwrap();
        r.ledger
            .set_owned(&super::test_support::world::ledger_context(&ra), p.local, 7)
            .unwrap();
        assert_eq!(p.available(), 0);
        assert_eq!(pair.owned_available(), 0);
        let provider_prices = p.family.prices.load(Ordering::SeqCst);
        let requester_prices = r.family.prices.load(Ordering::SeqCst);
        for stream in [3, 5] {
            let mut ra = original;
            ra.local = r.local;
            ra.peer = p.local;
            ra.operation.connection = [6; 32];
            ra.operation.stream = stream;
            let peer = super::acceptance::peer(ra);
            let request_grant = r
                .services
                .grant(r.local, services::authority::Action::Request, 16_777_216)
                .unwrap();
            let request_context = service_context(&request_grant);
            let observation_grant = r
                .services
                .grant(r.local, services::authority::Action::Observe, 16_777_216)
                .unwrap();
            let observation_context = service_context(&observation_grant);
            let retry = r
                .runtime
                .retry_requester(&request_context, &observation_context, original_key, peer)
                .unwrap();
            assert_eq!(retry.original(), original_key);
            assert_eq!(retry.peer().key, peer.key);
            assert_eq!(retry.maximum(), 7);
            assert!(retry.payment().transfer.is_none());
            let outgoing_grant = r
                .authority
                .grant(r.local, Action::Request, 16_777_216)
                .unwrap();
            let proposal = replay::request(
                &context(&outgoing_grant),
                r.authority.as_ref(),
                &retry,
                Limits::default(),
            )
            .unwrap();
            let pa = Association {
                local: p.local,
                peer: r.local,
                ..ra
            };
            let invoke = p
                .authority
                .grant(p.local, Action::Invoke, 16_777_216)
                .unwrap();
            let pc = context(&invoke);
            let recover = p
                .services
                .grant(p.local, services::authority::Action::Recover, 16_777_216)
                .unwrap();
            let recover_context = service_context(&recover);
            let observe = p
                .services
                .grant(p.local, services::authority::Action::Observe, 16_777_216)
                .unwrap();
            let observe_context = service_context(&observe);
            let cached = replay::provider(
                &pc,
                p.authority.as_ref(),
                pa,
                &proposal,
                Lookup {
                    registry: p.registry.as_ref(),
                    recover: &recover_context,
                    observe: &observe_context,
                    runtime: p.runtime.as_ref(),
                },
                Limits::default(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(cached.invocation().key, original_key);
            assert_eq!(cached.completion().price, 7);
            let mut ordering = Exchange::new(Direction::Remote);
            ordering.control(Direction::Remote, Kind::Propose).unwrap();
            ordering.cached_local(&cached).unwrap();
            ordering.end(Direction::Remote).unwrap();
            let mut remote = Exchange::new(Direction::Local);
            remote.control(Direction::Local, Kind::Propose).unwrap();
            remote.cached_remote(&retry).unwrap();
            remote.end(Direction::Local).unwrap();
            let mut sender =
                replay::Sender::new(&pc, p.authority.as_ref(), cached, Limits::default()).unwrap();
            assert_eq!(sender.proof().peer().key, peer.key);
            let mut queue = Outbound::new(16_384).unwrap();
            while sender.enqueue_next(&mut queue, 1000).unwrap() {}
            assert!(sender.queued());
            let incoming = r
                .authority
                .grant(r.local, Action::Invoke, 16_777_216)
                .unwrap();
            let ic = context(&incoming);
            let mut receiver =
                ResultReceiver::for_retry(&ic, r.authority.clone(), retry, Limits::default())
                    .unwrap();
            for record in drain(&mut queue) {
                match record {
                    Record::Control(bytes) => {
                        let message = Message::parse(&bytes, 65536).unwrap();
                        if message.kind() == Kind::ResultEnd {
                            ordering.control(Direction::Local, Kind::ResultEnd).unwrap();
                            remote.control(Direction::Remote, Kind::ResultEnd).unwrap();
                        }
                        receiver.control(&ic, ra, &message).unwrap();
                    }
                    body => {
                        ordering.body(Direction::Local).unwrap();
                        remote.body(Direction::Remote).unwrap();
                        receiver.body(&ic, ra, &body).unwrap();
                    }
                }
            }
            ordering.end(Direction::Local).unwrap();
            remote.end(Direction::Remote).unwrap();
            assert_eq!(ordering.close(true), Ok(Outcome::Complete));
            assert_eq!(remote.close(true), Ok(Outcome::Complete));
            let coordinate = r
                .services
                .grant(r.local, services::authority::Action::Coordinate, 16_777_216)
                .unwrap();
            let cc = service_context(&coordinate);
            receiver
                .finish(&ic, ra, true, &cc, r.runtime.as_ref())
                .unwrap();
            r.runtime.resume(&cc, original_key).unwrap();
            assert_eq!(p.family.executions(), 1);
            assert_eq!(p.family.prices.load(Ordering::SeqCst), provider_prices);
            assert_eq!(r.family.prices.load(Ordering::SeqCst), requester_prices);
            assert_eq!(p.available(), 0);
            assert_eq!(pair.owned_available(), 0);
        }
    }
}
// Probe grants are local read/recovery capabilities and cannot confer new invocation or spending authority.
fn probe(
    pair: &Pair<Messaging>,
    a: Association,
    message: &Message,
) -> Result<Option<services::replay::Cached>, Error> {
    let p = &pair.provider;
    let g = p
        .authority
        .grant(p.local, Action::Invoke, 16_777_216)
        .unwrap();
    let c = context(&g);
    let recover = p
        .services
        .grant(p.local, services::authority::Action::Recover, 16_777_216)
        .unwrap();
    let rc = service_context(&recover);
    let observe = p
        .services
        .grant(p.local, services::authority::Action::Observe, 16_777_216)
        .unwrap();
    let oc = service_context(&observe);
    replay::provider(
        &c,
        p.authority.as_ref(),
        a,
        message,
        Lookup {
            registry: p.registry.as_ref(),
            recover: &rc,
            observe: &oc,
            runtime: p.runtime.as_ref(),
        },
        Limits::default(),
    )
}
// The same replay ID cannot alter any original known field, ownership, issuer or economic binding; unrelated resources reveal no cached output.
#[test]
fn cached_results_require_original_caller_and_all_request_fields() {
    let pair = stacks();
    let p = &pair.provider;
    let r = &pair.requester;
    let input = Messaging::input(false);
    let (issued, request) = pair.propose(1, input.clone(), 7);
    let a = issued.association();
    let outgoing = pair.reserve(&issued, request).unwrap();
    let incoming = pair
        .accept(
            issued,
            a,
            &Message::parse(outgoing.accept(), 65536).unwrap(),
        )
        .unwrap();
    pair.commit_provider(incoming, &input);
    let original = super::acceptance::peer(outgoing.association()).key;
    let rg = r
        .services
        .grant(r.local, services::authority::Action::Request, 16_777_216)
        .unwrap();
    let rc = service_context(&rg);
    let og = r
        .services
        .grant(r.local, services::authority::Action::Observe, 16_777_216)
        .unwrap();
    let oc = service_context(&og);
    let mut ra = outgoing.association();
    ra.operation.stream = 3;
    let pa = Association {
        local: p.local,
        peer: r.local,
        ..ra
    };
    let retry = r
        .runtime
        .retry_requester(&rc, &oc, original, super::acceptance::peer(ra))
        .unwrap();
    let pg = r
        .authority
        .grant(r.local, Action::Request, 16_777_216)
        .unwrap();
    let message = replay::request(
        &context(&pg),
        r.authority.as_ref(),
        &retry,
        Limits::default(),
    )
    .unwrap();
    for case in 0..8 {
        let mut v = message.value().clone();
        match case {
            0 => {
                v["input"]["recipientSignature"] =
                    serde_json::json!({"$base64":super::base64::encode(&[6;64])})
            }
            1 => {
                v["input"]["messageId"] =
                    serde_json::json!({"$base64":super::base64::encode(&[6;32])})
            }
            2 => {
                v["creditIssuer"] =
                    serde_json::json!(crate::modules::identity::text::format(r.local))
            }
            3 => {
                v["publisherId"] =
                    serde_json::json!(crate::modules::identity::text::format(r.local))
            }
            4 => v["offeringId"] = serde_json::json!("different"),
            5 => v["maximumPrice"] = serde_json::json!("8"),
            6 => {
                v["transferId"] = serde_json::json!(super::base64::encode(&[1; 16]));
                v["transferFee"] = serde_json::json!("0");
            }
            _ => {
                v["input"]["payloadHash"] =
                    serde_json::json!({"$base64":super::base64::encode(&[6;32])})
            }
        }
        let changed = Message::parse(v.to_string().as_bytes(), 65536).unwrap();
        assert!(
            matches!(probe(&pair, pa, &changed), Err(Error::Conflict)),
            "case {case}"
        );
    }
    let mut wrong = pa;
    wrong.peer = crate::modules::identity::api::IdentityId([3; 32]);
    wrong.operation.requester = wrong.peer;
    assert!(probe(&pair, wrong, &message).unwrap().is_none());
    let mut v = message.value().clone();
    v["input"]["placementId"] = serde_json::json!({"$base64":super::base64::encode(&[7;32])});
    assert!(
        probe(
            &pair,
            pa,
            &Message::parse(v.to_string().as_bytes(), 65536).unwrap()
        )
        .unwrap()
        .is_none()
    );
    p.family.allow.store(false, Ordering::SeqCst);
    assert!(matches!(
        probe(&pair, pa, &message),
        Err(Error::Unsupported)
    ));
    assert_eq!(p.family.executions(), 1);
    assert_eq!(p.available(), 93);
    assert_eq!(pair.owned_available(), 93);
    let mut order = Exchange::new(Direction::Local);
    order.control(Direction::Local, Kind::Propose).unwrap();
    assert_eq!(
        order.control(Direction::Remote, Kind::Result),
        Err(Error::Invalid)
    );
}
// Matching uncommitted work stays uncertain, and a cached retry failure cannot release the original requester obligation.
#[test]
fn retry_keeps_uncertainty_and_rejects_cross_owner_permits() {
    let pair = stacks();
    let p = &pair.provider;
    let r = &pair.requester;
    let input = Messaging::input(false);
    let (issued, request) = pair.propose(1, input.clone(), 7);
    let proposal = Message::parse(request.bytes(), 65536).unwrap();
    let original = issued.association();
    let outgoing = pair.reserve(&issued, request).unwrap();
    let incoming = pair
        .accept(
            issued,
            original,
            &Message::parse(outgoing.accept(), 65536).unwrap(),
        )
        .unwrap();
    let mut pa = original;
    pa.operation.stream = 3;
    assert!(probe(&pair, pa, &proposal).unwrap().is_none());
    let pg = p
        .authority
        .grant(p.local, Action::Invoke, 16_777_216)
        .unwrap();
    let pc = context(&pg);
    let cg = p
        .services
        .grant(p.local, services::authority::Action::Coordinate, 16_777_216)
        .unwrap();
    let cc = service_context(&cg);
    let mut input_gate =
        super::input_transfer::InputTransfer::new(&pc, p.authority.clone(), incoming).unwrap();
    input_gate
        .marker(
            &pc,
            original,
            &Message::parse(br#"{"type":"input.end"}"#, 65536).unwrap(),
        )
        .unwrap();
    input_gate
        .end(&pc, original)
        .unwrap()
        .dispatch(&cc, p.runtime.as_ref())
        .unwrap();
    assert!(matches!(probe(&pair, pa, &proposal), Err(Error::Unknown)));
    let ra = Association {
        local: r.local,
        peer: p.local,
        ..pa
    };
    let peer = super::acceptance::peer(ra);
    let original_key = super::acceptance::peer(outgoing.association()).key;
    let request = r
        .services
        .grant(r.local, services::authority::Action::Request, 16_777_216)
        .unwrap();
    let rc = service_context(&request);
    let observe = r
        .services
        .grant(r.local, services::authority::Action::Observe, 16_777_216)
        .unwrap();
    let oc = service_context(&observe);
    let retry = r
        .runtime
        .retry_requester(&rc, &oc, original_key, peer)
        .unwrap();
    let other = stacks();
    let foreign = &other.requester;
    let cg = foreign
        .services
        .grant(
            foreign.local,
            services::authority::Action::Coordinate,
            16_777_216,
        )
        .unwrap();
    let result = services::runtime::RemoteResult {
        output: input,
        bytes: services::handler::Blob::new(vec![0], 1).unwrap(),
        price: 7,
    };
    assert_eq!(
        foreign
            .runtime
            .receive_replay(&service_context(&cg), &retry, peer, result),
        Err(services::Error::Denied)
    );
    let incoming = r
        .authority
        .grant(r.local, Action::Invoke, 16_777_216)
        .unwrap();
    let ic = context(&incoming);
    let mut receiver =
        ResultReceiver::for_retry(&ic, r.authority.clone(), retry, Limits::default()).unwrap();
    receiver
        .control(
            &ic,
            ra,
            &Message::parse(br#"{"type":"failure","code":"not_available"}"#, 65536).unwrap(),
        )
        .unwrap();
    let cg = r
        .services
        .grant(r.local, services::authority::Action::Coordinate, 16_777_216)
        .unwrap();
    assert_eq!(
        receiver.finish(&ic, ra, true, &service_context(&cg), r.runtime.as_ref()),
        Err(Error::Unavailable)
    );
    assert_eq!(
        r.runtime.status(&oc, original_key),
        Ok(Some(services::runtime::Phase::Executing))
    );
    assert_eq!(pair.owned_available(), 93);
    assert_eq!(p.family.executions(), 1);
}
