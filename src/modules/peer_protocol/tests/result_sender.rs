//! Durable provider results remain available after failed sends and cannot be read before commitment.
use super::{
    Error,
    authority::Action,
    contract::Limits,
    input_transfer::InputTransfer,
    message::Message,
    outbound::Outbound,
    record::{Decoder, Record},
    result_sender::ResultSender,
    test_support::{
        invocation::Pair,
        world::{context, service_context},
    },
};
use crate::modules::services::{self, value::Value};
// A real provider commits once; bounded transmission then survives queue refusal and dropped senders without changing accounting.
#[test]
fn only_committed_results_can_be_transmitted() {
    let pair = Pair::new(true);
    let input = Value::Bytes(vec![9; 5000]);
    let (issued, request) = pair.propose(1, input.clone(), 7);
    let a = issued.association();
    let outgoing = pair.reserve(&issued, request).unwrap();
    let accept = Message::parse(outgoing.accept(), 65536).unwrap();
    let incoming = pair.accept(issued, a, &accept).unwrap();
    let p = &pair.provider;
    let pg = p
        .authority
        .grant(p.local, Action::Invoke, 16_777_216)
        .unwrap();
    let c = context(&pg);
    let sg = p
        .services
        .grant(p.local, services::authority::Action::Observe, 16_777_216)
        .unwrap();
    let sc = service_context(&sg);
    let cg = p
        .services
        .grant(p.local, services::authority::Action::Coordinate, 16_777_216)
        .unwrap();
    let cc = service_context(&cg);
    assert!(matches!(
        ResultSender::new(
            &c,
            &p.authority,
            &sc,
            p.runtime.as_ref(),
            a,
            Limits::default()
        ),
        Err(Error::Unknown)
    ));
    let mut transfer = InputTransfer::new(&c, p.authority.clone(), incoming).unwrap();
    transfer
        .body(
            &c,
            a,
            &Record::Body {
                id: 0,
                offset: 0,
                data: vec![9; 5000],
            },
        )
        .unwrap();
    transfer
        .marker(
            &c,
            a,
            &Message::parse(b"{\"type\":\"input.end\"}", 65536).unwrap(),
        )
        .unwrap();
    transfer
        .end(&c, a)
        .unwrap()
        .dispatch(&cc, p.runtime.as_ref())
        .unwrap();
    let key = super::acceptance::peer(a).key;
    p.runtime.stage(&cc, key).unwrap();
    p.runtime.prepare(&cc, key).unwrap();
    assert!(matches!(
        ResultSender::new(
            &c,
            &p.authority,
            &sc,
            p.runtime.as_ref(),
            a,
            Limits::default()
        ),
        Err(Error::Unknown)
    ));
    p.runtime
        .decide(&cc, key, services::runtime::Decision::Commit)
        .unwrap();
    p.runtime.commit(&cc, key).unwrap();
    let mut sender = ResultSender::new(
        &c,
        &p.authority,
        &sc,
        p.runtime.as_ref(),
        a,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(sender.association(), a);
    assert_eq!(sender.completion().output, input);
    let mut tiny = Outbound::new(17).unwrap();
    assert_eq!(sender.enqueue_next(&mut tiny, 1000), Err(Error::Limit));
    assert!(!sender.queued());
    assert_eq!(tiny.pending(), 0);
    let mut queue = Outbound::new(16_384).unwrap();
    while sender.enqueue_next(&mut queue, 1000).unwrap() {}
    assert!(sender.queued());
    let mut decoder = Decoder::default();
    let mut records = Vec::new();
    while queue.pending() > 0 {
        let data = queue.front(31).unwrap().to_vec();
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
    assert_eq!(records.len(), 7);
    let Record::Control(first) = &records[0] else {
        panic!("missing result metadata")
    };
    let result = Message::parse(first, 65536).unwrap();
    assert_eq!(result.value()["finalPrice"], "7");
    assert_eq!(
        result.value()["output"],
        serde_json::json!({"$bytes":0,"length":"5000"})
    );
    let Record::Control(last) = records.last().unwrap() else {
        panic!("missing result.end")
    };
    assert_eq!(
        Message::parse(last, 65536).unwrap().kind(),
        super::message::Kind::ResultEnd
    );
    drop(sender);
    assert_eq!(p.runtime.result(&sc, key).unwrap().output, input);
    assert_eq!(p.available(), 93);
    // A different peer/stream cannot borrow this committed result even under a valid local grant.
    let mut wrong = a;
    wrong.peer = p.local;
    assert!(matches!(
        ResultSender::new(
            &c,
            &p.authority,
            &sc,
            p.runtime.as_ref(),
            wrong,
            Limits::default()
        ),
        Err(Error::Denied)
    ));
}
