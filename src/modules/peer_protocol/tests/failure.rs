//! Known failure meanings, concealed access and authoritative release remain distinct from uncertain outcomes.
use super::{
    Error,
    authority::Action,
    contract::Limits,
    failure::{self, AbortPorts, Code, Failure},
    message::Message,
    record::Record,
    result_receiver::ResultReceiver,
    test_support::{
        invocation::Pair,
        world::{context, service_context},
    },
};
use crate::modules::services::{self, value::Value};
// All specified machine codes survive serialization; diagnostics are bounded and unknown codes remain generic with no retry semantics.
#[test]
fn exact_codes_and_bounded_diagnostics() {
    for spelling in [
        "invalid_request",
        "unsupported",
        "not_available",
        "not_authorized",
        "terms_changed",
        "price_rejected",
        "insufficient_credits",
        "resource_limit",
        "conflict",
        "internal_error",
    ] {
        let code = Code::parse(spelling);
        assert_eq!(code.wire(), Some(spelling));
        let failure = Failure {
            code,
            message: Some("é".repeat(512)),
        };
        let Record::Control(bytes) = failure.record().unwrap() else {
            panic!("expected control")
        };
        assert_eq!(
            Failure::parse(&Message::parse(&bytes, 65536).unwrap()),
            Ok(failure)
        );
        assert_ne!(code.error(), Error::Unknown);
    }
    let future=Message::parse(br#"{"type":"failure","code":"retry_everything_immediately","message":"not an instruction"}"#,65536).unwrap();
    let failure = Failure::parse(&future).unwrap();
    assert_eq!(failure.code, Code::Unknown);
    assert_eq!(failure.code.error(), Error::Unavailable);
    assert_eq!(failure.record(), Err(Error::Unsupported));
    assert_eq!(
        Failure {
            code: Code::InternalError,
            message: Some("é".repeat(513))
        }
        .record(),
        Err(Error::Limit)
    );
    assert_eq!(Code::classify(Error::Denied, true), Ok(Code::NotAvailable));
    assert_eq!(
        Code::classify(Error::Denied, false),
        Ok(Code::NotAuthorized)
    );
    assert_eq!(Code::classify(Error::Unknown, false), Err(Error::Unknown));
    for error in [
        Error::Invalid,
        Error::Unsupported,
        Error::Limit,
        Error::Cancelled,
        Error::Conflict,
        Error::Insufficient,
        Error::Storage,
        Error::Unavailable,
    ] {
        assert!(Code::classify(error, false).unwrap().wire().is_some());
    }
    assert!(Failure::parse(&Message::parse(br#"{"type":"board.get"}"#, 65536).unwrap()).is_err());
}
// Provider failure requires acknowledged abort; committed work cannot be described as uncharged even if its response was lost.
#[test]
fn provider_failure_requires_authoritative_uncommitted_abort() {
    for committed in [false, true] {
        let pair = Pair::new(false);
        let input = Value::String("x".into());
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
        if committed {
            pair.commit_provider(incoming, &input);
        }
        let p = &pair.provider;
        let pg = p
            .authority
            .grant(p.local, Action::Invoke, 16_777_216)
            .unwrap();
        let cg = p
            .services
            .grant(p.local, services::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let cc = service_context(&cg);
        let og = p
            .services
            .grant(p.local, services::authority::Action::Observe, 16_777_216)
            .unwrap();
        let oc = service_context(&og);
        let failure = Failure {
            code: Code::NotAvailable,
            message: None,
        };
        let result = failure::abort_provider(
            &context(&pg),
            p.authority.as_ref(),
            a,
            AbortPorts {
                coordinate: &cc,
                observe: &oc,
                runtime: p.runtime.as_ref(),
            },
            failure.clone(),
        );
        if committed {
            assert_eq!(result, Err(Error::Conflict));
            assert_eq!(p.available(), 93);
        } else {
            assert_eq!(result, Ok(failure));
            assert_eq!(p.available(), 100);
        }
        assert_eq!(pair.owned_available(), 93);
    }
}
// Remote failure releases requester capacity only after normal END; interruption and failure after result.end preserve obligations.
#[test]
fn remote_failure_waits_for_normal_end_and_cannot_follow_success_marker() {
    for case in 0..4 {
        let pair = Pair::new(false);
        let (issued, request) = pair.propose(1, Value::String("x".into()), 7);
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
        let refusal =
            Message::parse(br#"{"type":"failure","code":"not_available"}"#, 65536).unwrap();
        if case >= 2 {
            receiver
                .control(
                    &c,
                    a,
                    &Message::parse(br#"{"type":"result","output":"x","finalPrice":"7"}"#, 65536)
                        .unwrap(),
                )
                .unwrap();
        }
        if case == 3 {
            receiver
                .control(
                    &c,
                    a,
                    &Message::parse(br#"{"type":"result.end"}"#, 65536).unwrap(),
                )
                .unwrap();
            assert_eq!(receiver.control(&c, a, &refusal), Err(Error::Invalid));
            assert!(
                receiver
                    .finish(&c, a, true, &cc, r.runtime.as_ref())
                    .is_err()
            );
        } else {
            receiver.control(&c, a, &refusal).unwrap();
            assert_eq!(receiver.failure().unwrap().code, Code::NotAvailable);
            assert_eq!(pair.owned_available(), 93);
            assert_eq!(
                receiver.finish(&c, a, case != 1, &cc, r.runtime.as_ref()),
                Err(if case == 1 {
                    Error::Unknown
                } else {
                    Error::Unavailable
                })
            );
        }
        assert_eq!(
            pair.owned_available(),
            if case == 0 || case == 2 { 100 } else { 93 }
        );
    }
}
