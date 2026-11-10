//! Complete result reception requires original provenance, terminal signals and independent Services validation.
use super::{
    Error,
    authority::Action,
    contract::Limits,
    message::Message,
    record::Record,
    result_receiver::ResultReceiver,
    test_support::{
        invocation::Pair,
        world::{context, service_context},
    },
};
use crate::modules::services::{self, value::Value};
// Normal completion stages the exact original output, and only subsequent durable commitment exposes application success.
#[test]
fn complete_result_is_applied_only_after_normal_end() {
    let pair = Pair::new(true);
    let input = Value::Bytes(vec![5; 5000]);
    let (issued, request) = pair.propose(1, input.clone(), 7);
    let pa = issued.association();
    let outgoing = pair.reserve(&issued, request).unwrap();
    let ra = outgoing.association();
    let incoming = pair
        .accept(
            issued,
            pa,
            &Message::parse(outgoing.accept(), 65536).unwrap(),
        )
        .unwrap();
    pair.commit_provider(incoming, &input);
    let r = &pair.requester;
    let pg = r
        .authority
        .grant(r.local, Action::Invoke, 16_777_216)
        .unwrap();
    let c = context(&pg);
    let cg = r
        .services
        .grant(r.local, services::authority::Action::Coordinate, 16_777_216)
        .unwrap();
    let cc = service_context(&cg);
    let observe = r
        .services
        .grant(r.local, services::authority::Action::Observe, 16_777_216)
        .unwrap();
    let oc = service_context(&observe);
    let mut receiver =
        ResultReceiver::new(&c, r.authority.clone(), &outgoing, Limits::default()).unwrap();
    let key = super::acceptance::peer(ra).key;
    let metadata = Message::parse(
        br#"{ "type":"result", "output":{"$bytes":0,"length":"5000"},"finalPrice":"7" }"#,
        65536,
    )
    .unwrap();
    receiver.control(&c, ra, &metadata).unwrap();
    receiver
        .body(
            &c,
            ra,
            &Record::Body {
                id: 0,
                offset: 0,
                data: vec![5; 4999],
            },
        )
        .unwrap();
    assert!(matches!(
        r.runtime.result(&oc, key),
        Err(services::Error::Unknown)
    ));
    receiver
        .body(
            &c,
            ra,
            &Record::Body {
                id: 0,
                offset: 4999,
                data: vec![5],
            },
        )
        .unwrap();
    receiver
        .control(
            &c,
            ra,
            &Message::parse(b"{\"type\":\"result.end\"}", 65536).unwrap(),
        )
        .unwrap();
    assert_eq!(
        r.runtime.status(&oc, key),
        Ok(Some(services::runtime::Phase::Executing))
    );
    assert_eq!(
        receiver.finish(&c, ra, true, &cc, r.runtime.as_ref()),
        Ok(ra)
    );
    assert_eq!(
        r.runtime.status(&oc, key),
        Ok(Some(services::runtime::Phase::Staged))
    );
    r.runtime.prepare(&cc, key).unwrap();
    r.runtime
        .decide(&cc, key, services::runtime::Decision::Commit)
        .unwrap();
    r.runtime.commit(&cc, key).unwrap();
    assert_eq!(r.runtime.result(&oc, key).unwrap().output, input);
    assert_eq!(pair.owned_available(), 93);
    assert_eq!(pair.provider.available(), 93);
}
// Missing/truncated/late or changed content cannot become successful output, and accepted obligations are retained on interruption.
#[test]
fn partial_wrong_and_abnormal_results_cannot_succeed() {
    for case in 0..8 {
        let pair = Pair::new(false);
        let (issued, request) = pair.propose(1, Value::String("original".into()), 7);
        let outgoing = pair.reserve(&issued, request).unwrap();
        let a = outgoing.association();
        let r = &pair.requester;
        let pg = r
            .authority
            .grant(r.local, Action::Invoke, 16_777_216)
            .unwrap();
        let c = context(&pg);
        let cg = r
            .services
            .grant(r.local, services::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let cc = service_context(&cg);
        let mut receiver =
            ResultReceiver::new(&c, r.authority.clone(), &outgoing, Limits::default()).unwrap();
        let metadata = Message::parse(
            br#"{"type":"result","output":"original","finalPrice":"7"}"#,
            65536,
        )
        .unwrap();
        let marker = Message::parse(br#"{"type":"result.end"}"#, 65536).unwrap();
        match case {
            0 => assert!(
                receiver
                    .finish(&c, a, true, &cc, r.runtime.as_ref())
                    .is_err()
            ),
            1 => {
                receiver.control(&c, a, &metadata).unwrap();
                assert!(
                    receiver
                        .finish(&c, a, true, &cc, r.runtime.as_ref())
                        .is_err()
                );
            }
            2 => {
                receiver.control(&c, a, &metadata).unwrap();
                receiver.control(&c, a, &marker).unwrap();
                assert_eq!(
                    receiver.finish(&c, a, false, &cc, r.runtime.as_ref()),
                    Err(Error::Unknown)
                );
            }
            3 => {
                let bad = Message::parse(
                    br#"{"type":"result","output":"changed","finalPrice":"7"}"#,
                    65536,
                )
                .unwrap();
                receiver.control(&c, a, &bad).unwrap();
                receiver.control(&c, a, &marker).unwrap();
                assert_eq!(
                    receiver.finish(&c, a, true, &cc, r.runtime.as_ref()),
                    Err(Error::Invalid)
                );
            }
            4 => {
                let bad = Message::parse(
                    br#"{"type":"result","output":"original","finalPrice":"8"}"#,
                    65536,
                )
                .unwrap();
                assert_eq!(receiver.control(&c, a, &bad), Err(Error::Invalid));
                assert!(
                    receiver
                        .finish(&c, a, true, &cc, r.runtime.as_ref())
                        .is_err()
                );
            }
            5 => {
                let mut wrong = a;
                wrong.operation.connection = [2; 32];
                assert_eq!(receiver.control(&c, wrong, &metadata), Err(Error::Denied));
                assert!(
                    receiver
                        .finish(&c, a, true, &cc, r.runtime.as_ref())
                        .is_err()
                );
            }
            6 => {
                receiver.control(&c, a, &metadata).unwrap();
                receiver.control(&c, a, &marker).unwrap();
                assert!(
                    receiver
                        .body(
                            &c,
                            a,
                            &Record::Body {
                                id: 0,
                                offset: 0,
                                data: vec![0]
                            }
                        )
                        .is_err()
                );
                assert!(
                    receiver
                        .finish(&c, a, true, &cc, r.runtime.as_ref())
                        .is_err()
                );
            }
            _ => {
                let detached = Message::parse(
                    br#"{"type":"result","output":{"$text":0,"length":"8"},"finalPrice":"7"}"#,
                    65536,
                )
                .unwrap();
                receiver.control(&c, a, &detached).unwrap();
                receiver
                    .body(
                        &c,
                        a,
                        &Record::Body {
                            id: 0,
                            offset: 0,
                            data: b"origina".to_vec(),
                        },
                    )
                    .unwrap();
                assert!(receiver.control(&c, a, &marker).is_err());
                assert!(
                    receiver
                        .finish(&c, a, true, &cc, r.runtime.as_ref())
                        .is_err()
                );
            }
        }
        assert_eq!(pair.owned_available(), 93);
    }
}
