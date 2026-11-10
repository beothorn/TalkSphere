//! Accepted input tests require both terminal signals over real reservations before effectful provider dispatch.
use super::{
    Error,
    authority::Action,
    contract::Limits,
    input_transfer::{InputSender, InputTransfer},
    message::Message,
    outbound::Outbound,
    record::{Decoder, Record},
    test_support::{
        invocation::Pair,
        world::{context, service_context},
    },
};
use crate::modules::services::{self, value::Value};
use std::sync::atomic::Ordering;
// Drain a queue using small Transport-like fragments and the independent incremental record parser.
fn drain(queue: &mut Outbound) -> Vec<Record> {
    let mut decoder = Decoder::default();
    let mut records = Vec::new();
    while queue.pending() > 0 {
        let bytes = queue.front(7).unwrap().to_vec();
        let mut offset = 0;
        while offset < bytes.len() {
            let (n, record) = decoder.push(&bytes[offset..]).unwrap();
            offset += n;
            if let Some(r) = record {
                records.push(r);
            }
        }
        queue.advance(bytes.len()).unwrap();
    }
    decoder.finish().unwrap();
    records
}
// Large accepted input streams in order and cannot execute after only bytes or input.end; END plus family validation unlocks dispatch.
#[test]
fn complete_input_markers_gate_effectful_dispatch() {
    let pair = Pair::new(true);
    let input = Value::Bytes(vec![3; 5000]);
    let (issued, request) = pair.propose(1, input.clone(), 7);
    let a = issued.association();
    let outgoing = pair.reserve(&issued, request).unwrap();
    let accept = Message::parse(outgoing.accept(), 65536).unwrap();
    let incoming = pair.accept(issued, a, &accept).unwrap();
    let pg = pair
        .provider
        .authority
        .grant(pair.provider.local, Action::Invoke, 16_777_216)
        .unwrap();
    let pc = context(&pg);
    let mut receiver = InputTransfer::new(&pc, pair.provider.authority.clone(), incoming).unwrap();
    let mut sender = InputSender::new(outgoing, Limits::default()).unwrap();
    assert_eq!(sender.outgoing().association().operation, a.operation);
    let mut queue = Outbound::new(512).unwrap();
    let mut seen_accept = false;
    let mut marker = false;
    while !sender.queued() {
        match sender.enqueue_next(&mut queue, 200) {
            Ok(true) => {}
            Err(Error::Limit) => {}
            other => panic!("unexpected sender result: {other:?}"),
        }
        for record in drain(&mut queue) {
            match record {
                Record::Control(bytes) => {
                    let m = Message::parse(&bytes, 65536).unwrap();
                    if !seen_accept {
                        assert_eq!(m.kind(), super::message::Kind::Accept);
                        seen_accept = true;
                    } else {
                        receiver.marker(&pc, a, &m).unwrap();
                        marker = true;
                    }
                }
                body => {
                    assert!(seen_accept);
                    assert!(!marker);
                    receiver.body(&pc, a, &body).unwrap();
                }
            }
            assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
        }
    }
    assert!(marker);
    assert!(!sender.enqueue_next(&mut queue, 200).unwrap());
    let complete = receiver.end(&pc, a).unwrap();
    assert_eq!(complete.value(), &input);
    assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
    let cg = pair
        .provider
        .services
        .grant(
            pair.provider.local,
            services::authority::Action::Coordinate,
            16_777_216,
        )
        .unwrap();
    assert_eq!(
        complete.dispatch(&service_context(&cg), pair.provider.runtime.as_ref()),
        Ok(a)
    );
    assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 1);
    // Work is staged locally but has no successful committed result or charge yet.
    assert_eq!(pair.provider.available(), 93);
}
// Premature markers, missing END, wrong IDs and data after the marker cannot dispatch; wrong-stream input cannot mutate this stream.
#[test]
fn incomplete_or_malformed_input_never_dispatches() {
    for case in 0..5 {
        let pair = Pair::new(case < 3);
        let input = if case < 3 {
            Value::Bytes(vec![1; 5000])
        } else {
            Value::String("x".into())
        };
        let (issued, request) = pair.propose(1, input, 7);
        let a = issued.association();
        let outgoing = pair.reserve(&issued, request).unwrap();
        let accept = Message::parse(outgoing.accept(), 65536).unwrap();
        let incoming = pair.accept(issued, a, &accept).unwrap();
        let pg = pair
            .provider
            .authority
            .grant(pair.provider.local, Action::Invoke, 16_777_216)
            .unwrap();
        let c = context(&pg);
        let mut transfer =
            InputTransfer::new(&c, pair.provider.authority.clone(), incoming).unwrap();
        let marker = Message::parse(b"{\"type\":\"input.end\"}", 65536).unwrap();
        match case {
            0 => assert!(transfer.marker(&c, a, &marker).is_err()),
            1 => assert!(transfer.end(&c, a).is_err()),
            2 => {
                assert!(
                    transfer
                        .body(
                            &c,
                            a,
                            &Record::Body {
                                id: 1,
                                offset: 0,
                                data: vec![1]
                            }
                        )
                        .is_err()
                );
                assert!(transfer.marker(&c, a, &marker).is_err());
            }
            3 => {
                transfer.marker(&c, a, &marker).unwrap();
                assert!(
                    transfer
                        .body(
                            &c,
                            a,
                            &Record::Body {
                                id: 0,
                                offset: 0,
                                data: vec![1]
                            }
                        )
                        .is_err()
                );
                assert!(transfer.end(&c, a).is_err());
            }
            _ => {
                let mut wrong = a;
                wrong.operation.stream = 4;
                assert_eq!(transfer.marker(&c, wrong, &marker), Err(Error::Denied));
                transfer.marker(&c, a, &marker).unwrap();
                assert!(transfer.marker(&c, a, &marker).is_err());
                assert!(transfer.end(&c, a).is_err());
            }
        }
        assert_eq!(pair.provider.family.executions.load(Ordering::SeqCst), 0);
        assert_eq!(pair.provider.available(), 93);
    }
}
