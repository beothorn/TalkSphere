//! Challenge-only paid checks use real module reservations and local timing, never provider correctness claims or private expected bytes.
use super::{
    Error,
    authority::Action,
    checks::{self, clock::Clock},
    contract::Limits,
    message::Message,
    outbound::Outbound,
    record::{Decoder, Record},
    test_support::{
        checks::{Checks, ScriptClock},
        world::{context, service_context},
    },
};
use crate::modules::services::{self, handler::Check, value::Value};
use std::sync::{Arc, atomic::Ordering};
// Fragment a complete queue independently of the producer and retain only complete logical records.
fn drain(queue: &mut Outbound) -> Vec<Record> {
    let mut decoder = Decoder::default();
    let mut records = Vec::new();
    while queue.pending() > 0 {
        let bytes = queue.front(9).unwrap().to_vec();
        let mut offset = 0;
        while offset < bytes.len() {
            let (n, r) = decoder.push(&bytes[offset..]).unwrap();
            offset += n;
            if let Some(r) = r {
                records.push(r);
            }
        }
        queue.advance(bytes.len()).unwrap();
    }
    decoder.finish().unwrap();
    records
}
// Well-formed wrong evidence is paid fail; private expectations are consulted only after complete normal evidence and local commitment.
#[test]
fn paid_evidence_is_challenge_only_and_locally_verified() {
    for expected in *b"iX" {
        let f = Checks::new(false);
        let (issued, request) = f.quote(1, Value::Uint64(0), 3).unwrap();
        assert_eq!(issued.price(), 3);
        assert_eq!(issued.id(), [1; 32]);
        let proposal = Message::parse(request.bytes(), 65536).unwrap();
        assert_eq!(proposal.value()["mode"], "check");
        assert_eq!(proposal.value()["input"], "0");
        assert_eq!(f.pair.provider.available(), 100);
        let outgoing = f.reserve(&issued, request).unwrap();
        let incoming = f.accept(issued, outgoing.accept()).unwrap();
        let pa = incoming.association();
        let ra = outgoing.association();
        assert_eq!(incoming.invocation().price, 3);
        assert_eq!(f.pair.provider.available(), 97);
        assert_eq!(f.pair.owned_available(), 97);
        let p = &f.pair.provider;
        let r = &f.pair.requester;
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
        let mut input = checks::input::Receiver::new(
            &pc,
            p.authority.clone(),
            incoming,
            Arc::new(ScriptClock::new(&[0])),
        )
        .unwrap();
        let mut sent = checks::input::Sender::new(outgoing, Limits::default()).unwrap();
        let mut queue = Outbound::new(1024).unwrap();
        while sent.enqueue_next(&mut queue).unwrap() {}
        let rc = r
            .services
            .grant(r.local, services::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let rcc = service_context(&rc);
        let clock = Arc::new(ScriptClock::new(&[10000, 10500]));
        assert!(
            sent.mark_sent(&queue, &rcc, r.runtime.as_ref(), clock.as_ref())
                .is_err()
        );
        let records = drain(&mut queue);
        assert_eq!(records.len(), 2);
        sent.mark_sent(&queue, &rcc, r.runtime.as_ref(), clock.as_ref())
            .unwrap();
        assert!(
            sent.mark_sent(&queue, &rcc, r.runtime.as_ref(), clock.as_ref())
                .is_err()
        );
        assert_eq!(p.family.evidence_calls.load(Ordering::SeqCst), 0);
        let Record::Control(marker) = &records[1] else {
            panic!("expected input.end")
        };
        input
            .marker(&pc, pa, &Message::parse(marker, 65536).unwrap())
            .unwrap();
        assert_eq!(p.family.evidence_calls.load(Ordering::SeqCst), 0);
        let dispatched = input.end(&pc, pa, &cc, p.runtime.as_ref()).unwrap();
        assert_eq!(p.family.evidence_calls.load(Ordering::SeqCst), 1);
        assert_eq!(*p.family.seen.lock().unwrap(), vec![Value::Uint64(0)]);
        let mut evidence = f.commit(dispatched, pa).unwrap();
        assert_eq!(evidence.evidence(), &Value::Bytes(vec![b'i']));
        assert_eq!(evidence.association(), pa);
        let mut queue = Outbound::new(1024).unwrap();
        while evidence.enqueue_next(&mut queue, 100).unwrap() {}
        assert!(evidence.queued());
        let received = drain(&mut queue);
        let ig = r
            .authority
            .grant(r.local, Action::Invoke, 16_777_216)
            .unwrap();
        let ic = context(&ig);
        let mut receiver = checks::evidence::Receiver::new(
            &ic,
            r.authority.clone(),
            sent.outgoing(),
            Limits::default(),
            clock,
        )
        .unwrap();
        for record in received {
            match record {
                Record::Control(bytes) => {
                    let message = Message::parse(&bytes, 65536).unwrap();
                    assert!(message.value()["output"].get("outcome").is_none());
                    receiver.control(&ic, ra, &message).unwrap();
                }
                body => receiver.body(&ic, ra, &body).unwrap(),
            }
        }
        receiver
            .finish(&ic, ra, true, &rcc, r.runtime.as_ref())
            .unwrap();
        let key = super::acceptance::peer(ra).key;
        r.runtime.prepare(&rcc, key).unwrap();
        r.runtime
            .decide(&rcc, key, services::runtime::Decision::Commit)
            .unwrap();
        r.runtime.commit(&rcc, key).unwrap();
        let og = r
            .services
            .grant(r.local, services::authority::Action::Observe, 16_777_216)
            .unwrap();
        let oc = service_context(&og);
        assert_eq!(
            r.runtime
                .check_outcome(&oc, key, Some(&Value::Bytes(vec![expected]))),
            Ok(if expected == b'i' {
                Check::Pass
            } else {
                Check::Fail
            })
        );
        assert_eq!(
            r.runtime.check_outcome(&oc, key, None),
            Ok(Check::Inconclusive)
        );
        assert_eq!(f.pair.provider.available(), 97);
        assert_eq!(f.pair.owned_available(), 97);
    }
}
// Local platform epochs remain comparable within one instance and finite scripted clocks fail on unexpected extra reads.
#[test]
fn explicit_check_clocks_are_monotonic_and_fail_closed() {
    let clock = checks::clock::SystemClock::new().unwrap();
    let (a, s) = clock.mark().unwrap();
    let (b, t) = clock.mark().unwrap();
    assert!(b >= a);
    assert_eq!(s.epoch, t.epoch);
    assert!(t.milliseconds >= s.milliseconds);
    assert_eq!(ScriptClock::new(&[]).mark(), Err(Error::Unavailable));
}
// Invalid quote fields, wrong attribution and insufficient/hash-mismatched consent never allocate check holds.
#[test]
fn check_quote_validation_and_local_consent_fail_closed() {
    for case in 0..10 {
        let f = Checks::new(false);
        let (issued, request) = f.quote(1, Value::Uint64(0), 3).unwrap();
        let r = &f.pair.requester;
        let mut wire = Message::parse(issued.bytes(), 65536)
            .unwrap()
            .value()
            .clone();
        let mut a = super::contract::Association {
            local: r.local,
            peer: f.pair.provider.local,
            ..issued.association()
        };
        match case {
            0 => wire["maximumPrice"] = serde_json::json!("4"),
            1 => wire["priceMode"] = serde_json::json!("metered"),
            2 => wire["pricingInputs"] = serde_json::json!({"payloadSize":"1"}),
            3 => wire["releaseCredits"] = serde_json::json!("1"),
            4 => {
                wire["offering"]["checkPrice"] = serde_json::json!(4);
                wire["maximumPrice"] = serde_json::json!("4");
            }
            5 => wire["offering"]["description"] = serde_json::json!("Different logical contract."),
            6 => {
                wire["creditIssuer"] =
                    serde_json::json!(crate::modules::identity::text::format(r.local))
            }
            7 => {
                wire["transferId"] = serde_json::json!(super::base64::encode(&[1; 16]));
                wire["transferFee"] = serde_json::json!("0");
            }
            8 => a.operation.stream += 2,
            _ => a.operation.connection = [0; 32],
        }
        let pg = r
            .authority
            .grant(r.local, Action::Invoke, 16_777_216)
            .unwrap();
        let sg = r
            .services
            .grant(r.local, services::authority::Action::Request, 16_777_216)
            .unwrap();
        let message = Message::parse(wire.to_string().as_bytes(), 65536).unwrap();
        assert!(
            f.requester
                .validate(&context(&pg), &service_context(&sg), request, a, &message)
                .is_err(),
            "case {case}"
        );
        assert_eq!(f.pair.owned_available(), 100);
        assert_eq!(f.pair.provider.available(), 100);
    }
    for wrong_hash in [false, true] {
        let f = Checks::new(false);
        let (issued, request) = f.quote(1, Value::Uint64(0), 3).unwrap();
        let r = &f.pair.requester;
        let pi = r
            .authority
            .grant(r.local, Action::Invoke, 16_777_216)
            .unwrap();
        let pr = r
            .authority
            .grant(r.local, Action::Request, 16_777_216)
            .unwrap();
        let sg = r
            .services
            .grant(r.local, services::authority::Action::Request, 16_777_216)
            .unwrap();
        let a = super::contract::Association {
            local: r.local,
            peer: f.pair.provider.local,
            ..issued.association()
        };
        let candidate = f
            .requester
            .validate(
                &context(&pi),
                &service_context(&sg),
                request,
                a,
                &Message::parse(issued.bytes(), 65536).unwrap(),
            )
            .unwrap();
        assert_eq!(candidate.price(), 3);
        assert_eq!(candidate.bytes(), issued.bytes());
        let hash = if wrong_hash {
            [0; 32]
        } else {
            candidate.hash()
        };
        assert!(
            f.requester
                .consent(
                    &context(&pr),
                    candidate,
                    hash,
                    if wrong_hash { 3 } else { 2 }
                )
                .is_err()
        );
        assert_eq!(f.pair.owned_available(), 100);
    }
    let f = Checks::new(false);
    assert!(f.quote(1, Value::Uint64(5), 3).is_err());
    assert!(
        f.quote(
            1,
            Value::Object(vec![("expected".into(), Value::Bytes(vec![42]))]),
            3
        )
        .is_err()
    );
    assert!(f.quote(1, Value::Uint64(0), 2).is_err());
}
struct PastClock;
impl Clock for PastClock {
    // A trusted past marker proves END cannot restart the already elapsed one-second response interval.
    fn mark(&self) -> Result<(std::time::Instant, services::checks::Stamp), Error> {
        Ok((
            std::time::Instant::now() - std::time::Duration::from_secs(2),
            services::checks::Stamp {
                epoch: [9; 32],
                milliseconds: 0,
            },
        ))
    }
}
// Provider evidence cannot run without exact terminal order, or extend its deadline by delaying requester END.
#[test]
fn check_input_terminals_and_marker_deadline_gate_evidence() {
    for case in 0..4 {
        let f = Checks::new(false);
        let (issued, request) = f.quote(1, Value::Uint64(0), 3).unwrap();
        let outgoing = f.reserve(&issued, request).unwrap();
        let incoming = f.accept(issued, outgoing.accept()).unwrap();
        let a = incoming.association();
        let p = &f.pair.provider;
        let pg = p
            .authority
            .grant(p.local, Action::Invoke, 16_777_216)
            .unwrap();
        let c = context(&pg);
        let cg = p
            .services
            .grant(p.local, services::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let cc = service_context(&cg);
        let clock: Arc<dyn Clock> = if case == 3 {
            Arc::new(PastClock)
        } else {
            Arc::new(ScriptClock::new(&[0]))
        };
        let mut input =
            checks::input::Receiver::new(&c, p.authority.clone(), incoming, clock).unwrap();
        let marker = Message::parse(br#"{"type":"input.end"}"#, 65536).unwrap();
        match case {
            0 => {}
            1 => {
                input.marker(&c, a, &marker).unwrap();
                assert_eq!(input.marker(&c, a, &marker), Err(Error::Invalid));
            }
            2 => assert_eq!(
                input.marker(
                    &c,
                    a,
                    &Message::parse(br#"{"type":"result.end"}"#, 65536).unwrap()
                ),
                Err(Error::Invalid)
            ),
            _ => input.marker(&c, a, &marker).unwrap(),
        }
        assert!(input.end(&c, a, &cc, p.runtime.as_ref()).is_err());
        assert_eq!(p.family.evidence_calls.load(Ordering::SeqCst), 0);
    }
}
// Provider outcome claims, incomplete/abnormal evidence and a complete late response cannot authorize requester payment.
#[test]
fn late_missing_or_provider_claimed_evidence_cannot_charge() {
    for case in 0..4 {
        let f = Checks::new(false);
        let (issued, request) = f.quote(1, Value::Uint64(0), 3).unwrap();
        let outgoing = f.reserve(&issued, request).unwrap();
        let a = outgoing.association();
        let r = &f.pair.requester;
        let cg = r
            .services
            .grant(r.local, services::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let cc = service_context(&cg);
        r.runtime
            .check_sent(
                &cc,
                super::acceptance::peer(a).key,
                services::checks::Stamp {
                    epoch: [4; 32],
                    milliseconds: 100,
                },
            )
            .unwrap();
        let pg = r
            .authority
            .grant(r.local, Action::Invoke, 16_777_216)
            .unwrap();
        let c = context(&pg);
        let mut receiver = checks::evidence::Receiver::new(
            &c,
            r.authority.clone(),
            &outgoing,
            Limits::default(),
            Arc::new(ScriptClock::new(&[1101])),
        )
        .unwrap();
        if case == 0 {
            let claimed = Message::parse(
                br#"{"type":"result","output":{"outcome":"pass"},"finalPrice":"3"}"#,
                65536,
            )
            .unwrap();
            assert_eq!(receiver.control(&c, a, &claimed), Err(Error::Invalid));
        } else {
            receiver
                .control(
                    &c,
                    a,
                    &Message::parse(
                        br#"{"type":"result","output":{"$base64":"aQ=="},"finalPrice":"3"}"#,
                        65536,
                    )
                    .unwrap(),
                )
                .unwrap();
            if case != 1 {
                receiver
                    .control(
                        &c,
                        a,
                        &Message::parse(br#"{"type":"result.end"}"#, 65536).unwrap(),
                    )
                    .unwrap();
            }
        }
        assert!(
            receiver
                .finish(&c, a, case != 2, &cc, r.runtime.as_ref())
                .is_err()
        );
        assert_eq!(f.pair.owned_available(), 97);
        r.runtime
            .check_timeout(
                &cc,
                super::acceptance::peer(a).key,
                services::checks::Stamp {
                    epoch: [4; 32],
                    milliseconds: 1101,
                },
            )
            .unwrap();
        assert_eq!(f.pair.owned_available(), 100);
    }
}
// An ordinary local verifier has no remote check capability, and incoming entropy failure cannot manufacture predictable quote IDs.
#[test]
fn local_only_checks_and_entropy_failure_are_explicit() {
    let pair = super::test_support::invocation::Pair::new(false);
    let r = &pair.requester;
    let owner = checks::Owner::new(checks::Config {
        local: r.local,
        authority: r.authority.clone(),
        services: r.services.clone(),
        registry: r.registry.clone(),
        limits: Limits::default(),
        gate: r.gate.clone(),
        entropy: Arc::new(super::quote::SystemEntropy),
    })
    .unwrap();
    let (pa, selected, _) = pair.provider.proposal(1, &Value::String("x".into()));
    let ra = super::contract::Association {
        local: r.local,
        peer: pair.provider.local,
        ..pa
    };
    let pg = r
        .authority
        .grant(r.local, Action::Request, 16_777_216)
        .unwrap();
    let sg = r
        .services
        .grant(r.local, services::authority::Action::Request, 16_777_216)
        .unwrap();
    assert!(matches!(
        owner.request(
            &context(&pg),
            &service_context(&sg),
            ra,
            selected,
            Value::String("private expectation is not a remote challenge".into()),
            super::requester::Spending {
                maximum: 3,
                payment: Default::default()
            }
        ),
        Err(Error::Unsupported)
    ));
    struct NoRandom;
    impl super::quote::Entropy for NoRandom {
        // An unavailable random source never emits a zero, hash-derived or sequential fallback identifier.
        fn quote_id(&self) -> Result<[u8; 32], Error> {
            Err(Error::Unavailable)
        }
    }
    let f = Checks::new(false);
    let (_issued, request) = f.quote(1, Value::Uint64(0), 3).unwrap();
    let p = &f.pair.provider;
    let owner = checks::Owner::new(checks::Config {
        local: p.local,
        authority: p.authority.clone(),
        services: p.services.clone(),
        registry: p.registry.clone(),
        limits: Limits::default(),
        gate: p.gate.clone(),
        entropy: Arc::new(NoRandom),
    })
    .unwrap();
    let (a, selection, _) = p.proposal(1, &Value::String("unused".into()));
    let pg = p
        .authority
        .grant(p.local, Action::Invoke, 16_777_216)
        .unwrap();
    let c = context(&pg);
    let sg = p
        .services
        .grant(p.local, services::authority::Action::Invoke, 16_777_216)
        .unwrap();
    let sc = service_context(&sg);
    let authorizer = super::proposal::Authorizer::new(
        p.local,
        p.authority.clone(),
        p.services.clone(),
        p.registry.clone(),
        Limits::default(),
        p.gate.clone(),
    )
    .unwrap();
    let proposal = authorizer
        .authorize(
            &c,
            &sc,
            a,
            selection,
            &Message::parse(request.bytes(), 65536).unwrap(),
        )
        .unwrap();
    let bg = p
        .board_authority
        .grant(
            p.local,
            crate::modules::board::authority::Action::Select,
            16_777_216,
        )
        .unwrap();
    let bc = crate::modules::board::contract::Context {
        grant: &bg,
        deadline: c.deadline,
        cancelled: false,
    };
    let current = services::quote::BoardCurrent {
        owner: &p.board,
        context: &bc,
    };
    assert!(matches!(
        owner.issue(&c, &sc, proposal, &current, c.deadline),
        Err(Error::Unavailable)
    ));
    assert_eq!(p.available(), 100);
    assert_eq!(p.family.evidence_calls.load(Ordering::SeqCst), 0);
}
// Included evidence consumes only complete responses; definite incomplete sends release capacity while uncertain delivery retains its durable guard.
#[test]
fn included_allowance_tracks_complete_and_uncertain_responses() {
    use services::checks::{Allowance, Delivery};
    for delivery in [Delivery::Complete, Delivery::Incomplete, Delivery::Unknown] {
        let f = Checks::new(true);
        let (issued, request) = f.quote(1, Value::Uint64(0), 0).unwrap();
        assert_eq!(issued.price(), 0);
        let wire = Message::parse(issued.bytes(), 65536).unwrap();
        assert!(wire.value().get("transferId").is_none());
        let outgoing = f.reserve(&issued, request).unwrap();
        let incoming = f.accept(issued, outgoing.accept()).unwrap();
        let pa = incoming.association();
        let ra = outgoing.association();
        let p = &f.pair.provider;
        let r = &f.pair.requester;
        assert_eq!(p.available(), 100);
        assert_eq!(f.pair.owned_available(), 100);
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
        let po = p
            .services
            .grant(p.local, services::authority::Action::Observe, 16_777_216)
            .unwrap();
        let poc = service_context(&po);
        let rc = r
            .services
            .grant(r.local, services::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let rcc = service_context(&rc);
        let ro = r
            .services
            .grant(r.local, services::authority::Action::Observe, 16_777_216)
            .unwrap();
        let roc = service_context(&ro);
        let pk = super::acceptance::peer(pa).key;
        let rk = super::acceptance::peer(ra).key;
        assert_eq!(p.runtime.check_allowance(&poc, pk), Ok(Allowance::Reserved));
        r.runtime
            .check_sent(
                &rcc,
                rk,
                services::checks::Stamp {
                    epoch: [4; 32],
                    milliseconds: 100,
                },
            )
            .unwrap();
        let mut input = checks::input::Receiver::new(
            &pc,
            p.authority.clone(),
            incoming,
            Arc::new(ScriptClock::new(&[0])),
        )
        .unwrap();
        input
            .marker(
                &pc,
                pa,
                &Message::parse(br#"{"type":"input.end"}"#, 65536).unwrap(),
            )
            .unwrap();
        let dispatched = input.end(&pc, pa, &cc, p.runtime.as_ref()).unwrap();
        let mut sender = f.commit(dispatched, pa).unwrap();
        assert_eq!(p.runtime.check_allowance(&poc, pk), Ok(Allowance::Reserved));
        let mut queue = Outbound::new(1024).unwrap();
        if delivery != Delivery::Incomplete {
            while sender.enqueue_next(&mut queue, 100).unwrap() {}
        }
        let records = drain(&mut queue);
        if delivery == Delivery::Complete {
            let ig = r
                .authority
                .grant(r.local, Action::Invoke, 16_777_216)
                .unwrap();
            let ic = context(&ig);
            let mut received = checks::evidence::Receiver::new(
                &ic,
                r.authority.clone(),
                &outgoing,
                Limits::default(),
                Arc::new(ScriptClock::new(&[200])),
            )
            .unwrap();
            for record in records {
                match record {
                    Record::Control(bytes) => received
                        .control(&ic, ra, &Message::parse(&bytes, 65536).unwrap())
                        .unwrap(),
                    body => received.body(&ic, ra, &body).unwrap(),
                }
            }
            received
                .finish(&ic, ra, true, &rcc, r.runtime.as_ref())
                .unwrap();
            assert_eq!(r.runtime.check_allowance(&roc, rk), Ok(Allowance::Consumed));
            r.runtime.prepare(&rcc, rk).unwrap();
            r.runtime
                .decide(&rcc, rk, services::runtime::Decision::Commit)
                .unwrap();
            r.runtime.commit(&rcc, rk).unwrap();
        } else {
            r.runtime
                .check_timeout(
                    &rcc,
                    rk,
                    services::checks::Stamp {
                        epoch: [4; 32],
                        milliseconds: 1101,
                    },
                )
                .unwrap();
            assert_eq!(r.runtime.check_allowance(&roc, rk), Ok(Allowance::Released));
        }
        sender
            .delivery(&cc, p.runtime.as_ref(), &queue, delivery)
            .unwrap();
        let expected = match delivery {
            Delivery::Complete => Allowance::Consumed,
            Delivery::Incomplete => Allowance::Released,
            Delivery::Unknown => Allowance::Unknown,
        };
        assert_eq!(p.runtime.check_allowance(&poc, pk), Ok(expected));
        assert_eq!(p.available(), 100);
        assert_eq!(f.pair.owned_available(), 100);
        let (next, request) = f.quote(2, Value::Uint64(0), 0).unwrap();
        if delivery == Delivery::Complete {
            assert!(matches!(f.reserve(&next, request), Err(Error::Limit)));
        } else {
            let outgoing = f.reserve(&next, request).unwrap();
            let result = f.accept(next, outgoing.accept());
            if delivery == Delivery::Incomplete {
                assert!(result.is_ok());
            } else {
                assert!(matches!(result, Err(Error::Limit)));
            }
        }
    }
}

// Original Transport receipt time must survive worker queue delay, without consulting a fresh clock at dispatch.
#[test]
fn original_marker_receipt_deadline_cannot_restart_on_a_worker() {
    use std::time::{Duration, Instant};
    let f = Checks::new(false);
    let (issued, request) = f.quote(1, Value::Uint64(0), 3).unwrap();
    let outgoing = f.reserve(&issued, request).unwrap();
    let incoming = f.accept(issued, outgoing.accept()).unwrap();
    let a = incoming.association();
    let p = &f.pair.provider;
    let invoke = p
        .authority
        .grant(p.local, Action::Invoke, 16_777_216)
        .unwrap();
    let request = p
        .authority
        .grant(p.local, Action::Request, 16_777_216)
        .unwrap();
    let coordinate = p
        .services
        .grant(p.local, services::authority::Action::Coordinate, 16_777_216)
        .unwrap();
    let pc = context(&invoke);
    let cc = service_context(&coordinate);
    let mut input = checks::input::Receiver::new(
        &pc,
        p.authority.clone(),
        incoming,
        Arc::new(ScriptClock::new(&[])),
    )
    .unwrap();
    let marker = Message::parse(br#"{"type":"input.end"}"#, 65536).unwrap();
    assert_eq!(
        input.marker(&context(&request), a, &marker),
        Err(Error::Denied)
    );
    input
        .marker_at(&pc, a, &marker, Instant::now() - Duration::from_secs(60))
        .unwrap();
    assert!(matches!(
        input.end(&pc, a, &cc, p.runtime.as_ref()),
        Err(Error::Cancelled)
    ));
    assert_eq!(p.family.evidence_calls.load(Ordering::SeqCst), 0);
}
