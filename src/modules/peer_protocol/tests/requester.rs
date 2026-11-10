//! Real requester Registry validation of exact provider quotes before consent or reservation.
use super::{
    Error,
    authority::Action,
    base64,
    contract::{Association, Limits},
    message::Message,
    requester::{Client, Spending},
    test_support::world::{World, context, service_context},
};
use crate::modules::services::{self, runtime::Payment, value::Value};
use serde_json::{Value as Json, json};
use std::{sync::Arc, sync::atomic::Ordering};
// A requester-only installation understands this literal custom contract without installing its provider side.
fn client(w: &World) -> Client {
    let registry = Arc::new(
        services::registry::Registry::new(
            w.requester,
            w.services.clone(),
            Default::default(),
            Arc::new(services::registry::NoStandards),
        )
        .unwrap(),
    );
    let register = w
        .services
        .grant(
            w.requester,
            services::authority::Action::Register,
            16_777_216,
        )
        .unwrap();
    registry
        .register(&service_context(&register), w.family.clone(), None)
        .unwrap();
    Client::new(
        w.requester,
        w.authority.clone(),
        w.services.clone(),
        registry,
        Limits::default(),
        Default::default(),
    )
    .unwrap()
}
// Literal provider fields are independent from the provider quote encoder and may contain harmless whitespace.
fn quote(
    w: &World,
    stream: u32,
) -> (
    Association,
    crate::modules::board::selection::Selection,
    Json,
) {
    let (a, selected, _) = w.proposal(stream, &Value::String("local secret".into()));
    let a = Association {
        local: w.requester,
        peer: w.local,
        ..a
    };
    let wire = json!({"type":"quote","quoteId":base64::encode(&[6;32]),"offering":selected.offering().source,"creditIssuer":crate::modules::identity::text::format(w.local),"priceMode":"fixed","maximumPrice":"7","pricingInputs":{}});
    (a, selected, wire)
}
// Changed supported prices require explicit exact-byte consent, with no implicit reservation or provider execution.
#[test]
fn independent_quote_and_explicit_consent() {
    let w = World::new(false);
    let client = client(&w);
    let request = w
        .authority
        .grant(w.requester, Action::Request, 16_777_216)
        .unwrap();
    let incoming = w
        .authority
        .grant(w.requester, Action::Invoke, 16_777_216)
        .unwrap();
    let services = w
        .services
        .grant(
            w.requester,
            services::authority::Action::Request,
            16_777_216,
        )
        .unwrap();
    let c = context(&request);
    let ic = context(&incoming);
    let sc = service_context(&services);
    let (a, selection, mut wire) = quote(&w, 1);
    let original = client
        .request(
            &c,
            &sc,
            a,
            selection,
            Value::String("local secret".into()),
            Spending {
                maximum: 10,
                payment: Payment::default(),
            },
        )
        .unwrap();
    let proposed = Message::parse(original.bytes(), 65536).unwrap();
    assert_eq!(proposed.value()["maximumPrice"], "10");
    assert_eq!(proposed.value()["input"], "local secret");
    wire["offering"]["price"] = json!(8);
    wire["maximumPrice"] = json!("8");
    let bytes = serde_json::to_vec_pretty(&wire).unwrap();
    let message = Message::parse(&bytes, 65536).unwrap();
    let candidate = client.validate(&ic, &sc, original, a, &message).unwrap();
    assert_eq!(candidate.bytes(), bytes);
    assert_eq!(candidate.id(), [6; 32]);
    assert_eq!(candidate.price(), services::pricing::Price::Fixed(8));
    assert_eq!(candidate.association(), a);
    assert!(candidate.payment().transfer.is_none());
    assert_eq!(candidate.deadline(), c.deadline.min(sc.deadline));
    assert_eq!(candidate.selection().offering().source["price"], 8);
    use sha2::{Digest, Sha256};
    let hash: [u8; 32] = Sha256::digest(&bytes).into();
    assert_eq!(candidate.hash(), hash);
    let approved = client.consent(&c, candidate, hash, 8).unwrap();
    assert_eq!(approved.candidate().hash(), hash);
    assert_eq!(w.available(), 100);
    assert_eq!(w.family.executions.load(Ordering::SeqCst), 0);
}
// Each malformed or misattributed snapshot fails before spending; unchanged low prices still need correct local consent.
#[test]
fn quotes_cannot_change_contract_or_payment_or_fabricate_factors() {
    let w = World::new(false);
    let client = client(&w);
    let request = w
        .authority
        .grant(w.requester, Action::Request, 16_777_216)
        .unwrap();
    let incoming = w
        .authority
        .grant(w.requester, Action::Invoke, 16_777_216)
        .unwrap();
    let services = w
        .services
        .grant(
            w.requester,
            services::authority::Action::Request,
            16_777_216,
        )
        .unwrap();
    let c = context(&request);
    let ic = context(&incoming);
    let sc = service_context(&services);
    for case in 0..14 {
        let (a, s, mut wire) = quote(&w, case + 1);
        let original = client
            .request(
                &c,
                &sc,
                a,
                s,
                Value::String("local secret".into()),
                Spending {
                    maximum: 10,
                    payment: Payment::default(),
                },
            )
            .unwrap();
        let mut observed = a;
        match case {
            0 => wire["offering"]["id"] = json!("different"),
            1 => wire["offering"]["serviceVersion"] = json!(2),
            2 => wire["offering"]["description"] = json!("Different contract."),
            3 => wire["creditIssuer"] = json!(crate::modules::identity::text::format(w.requester)),
            4 => wire["maximumPrice"] = json!("6"),
            5 => wire["priceMode"] = json!("metered"),
            6 => wire["pricingInputs"] = json!({"invented":"1"}),
            7 => wire["releaseCredits"] = json!("1"),
            8 => {
                wire["transferId"] = json!(base64::encode(&[1; 16]));
                wire["transferFee"] = json!("0");
            }
            9 => {
                wire["offering"]["price"] = json!(11);
                wire["maximumPrice"] = json!("11");
            }
            10 => observed.operation.stream += 100,
            11 => observed.operation.connection = [3; 32],
            12 => observed.peer = w.requester,
            _ => wire["offering"]["preconditions"] = json!(["unrecognized"]),
        }
        let message = Message::parse(wire.to_string().as_bytes(), 65536).unwrap();
        assert!(
            client
                .validate(&ic, &sc, original, observed, &message)
                .is_err(),
            "case {case}"
        );
    }
    for case in 0..4 {
        let (a, s, wire) = quote(&w, 40 + case);
        let original = client
            .request(
                &c,
                &sc,
                a,
                s,
                Value::String("local secret".into()),
                Spending {
                    maximum: 10,
                    payment: Payment::default(),
                },
            )
            .unwrap();
        let message = Message::parse(wire.to_string().as_bytes(), 65536).unwrap();
        if case == 3 {
            assert!(matches!(
                client.validate(&c, &sc, original, a, &message),
                Err(Error::Denied)
            ));
            continue;
        }
        let candidate = client.validate(&ic, &sc, original, a, &message).unwrap();
        let hash = candidate.hash();
        let result = match case {
            0 => client.consent(&c, candidate, [0; 32], 10),
            1 => client.consent(&c, candidate, hash, 6),
            _ => {
                let mut expired = context(&request);
                expired.cancelled = true;
                client.consent(&expired, candidate, hash, 10)
            }
        };
        assert!(result.is_err());
    }
    assert_eq!(w.available(), 100);
    assert_eq!(w.family.executions.load(Ordering::SeqCst), 0);
}
