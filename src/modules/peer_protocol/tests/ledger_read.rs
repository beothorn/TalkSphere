//! Actual issuer queries disclose only caller balances and original/new-owner transfer evidence, without service dispatch or balance mutation.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Association, Limits, OperationKey},
    ledger_control::{
        query::{Request, Response},
        read,
    },
    message::Message,
    record::Record,
    test_support::{
        ledger::Issuer,
        world::{context, ledger_context},
    },
};
use crate::modules::{
    identity::{api::IdentityId, text},
    ledger,
};
use std::sync::Arc;
// Server-side associations come from trusted authentication; JSON owner hints cannot replace this caller.
fn association(f: &Issuer, caller: IdentityId) -> Association {
    Association {
        local: f.local,
        peer: caller,
        operation: OperationKey {
            requester: caller,
            connection: [8; 32],
            stream: 3,
        },
    }
}
// Bound real server Control permission and decode only complete constructed wire controls.
fn respond(f: &Issuer, a: Association, wire: serde_json::Value) -> Message {
    let pg = f.peer.grant(f.local, Action::Invoke, 16_777_216).unwrap();
    let lg = f
        .authority
        .grant(f.local, ledger::authority::Action::Control, 16_777_216)
        .unwrap();
    let message = Message::parse(wire.to_string().as_bytes(), 65536).unwrap();
    let Record::Control(bytes) = read::respond(
        &context(&pg),
        f.peer.as_ref(),
        f.ledger.as_ref(),
        &ledger_context(&lg),
        a,
        &message,
        Limits::default(),
    )
    .unwrap() else {
        panic!("expected control")
    };
    Message::parse(&bytes, 65536).unwrap()
}
// Owner and recipient can query the committed exact result; unrelated callers see the same absence code for existing or missing IDs.
#[test]
fn caller_balances_and_protected_transfer_results() {
    let f = Issuer::new();
    let original = f.commit();
    for (caller, expected) in [
        (f.owner, "91"),
        (f.recipient, "7"),
        (IdentityId([4; 32]), "0"),
    ] {
        let a = association(&f, caller);
        let result = respond(
            &f,
            a,
            serde_json::json!({"type":"ledger.balance","ownerId":text::format(f.owner)}),
        );
        assert_eq!(result.value()["balance"], expected);
    }
    for caller in [f.owner, f.recipient] {
        let result = respond(
            &f,
            association(&f, caller),
            serde_json::json!({"type":"ledger.transfer.get","ownerId":text::format(f.owner),"transferId":super::base64::encode(&f.order.transfer_id)}),
        );
        assert_eq!(result.value()["amount"], "9");
        assert_eq!(result.value()["fee"], "2");
        assert_eq!(result.value()["receivedAmount"], "7");
        assert_eq!(
            result.value()["ownerId"],
            text::format(original.order.owner)
        );
    }
    for id in [f.order.transfer_id, [2; 16]] {
        let reply = respond(
            &f,
            association(&f, IdentityId([4; 32])),
            serde_json::json!({"type":"ledger.transfer.get","ownerId":text::format(f.owner),"transferId":super::base64::encode(&id)}),
        );
        assert_eq!(reply.value()["code"], "not_available");
    }
    let read = f
        .authority
        .grant(f.local, ledger::authority::Action::Read, 16_777_216)
        .unwrap();
    let c = ledger_context(&read);
    assert_eq!(f.ledger.view(&c, f.owner).unwrap().issued, 91);
    assert_eq!(f.ledger.view(&c, f.recipient).unwrap().issued, 7);
}
// Exact original route and normal completion are required before creating opaque issuer evidence; balances remain observations only.
#[test]
fn query_completion_keeps_original_issuer_provenance() {
    let f = Issuer::new();
    f.commit();
    let a = Association {
        local: f.recipient,
        peer: f.local,
        operation: OperationKey {
            requester: f.recipient,
            connection: [9; 32],
            stream: 2,
        },
    };
    let auth = Arc::new(Authority::new());
    let outgoing = auth.grant(a.local, Action::Request, 16_777_216).unwrap();
    let incoming = auth.grant(a.local, Action::Invoke, 16_777_216).unwrap();
    let c = context(&outgoing);
    let ic = context(&incoming);
    for case in 0..5 {
        let request = Request::transfer(&c, auth.clone(), a, f.owner, f.order.transfer_id).unwrap();
        let raw = Message::parse(request.bytes(), 65536).unwrap();
        let mut reply = respond(&f, association(&f, f.recipient), raw.value().clone())
            .value()
            .clone();
        let mut observed = a;
        if case == 1 {
            observed.operation.connection = [0; 32];
        }
        if case == 2 {
            reply["receivedAmount"] = serde_json::json!("8");
        }
        if case == 3 {
            reply["transferId"] = serde_json::json!(super::base64::encode(&[9; 16]));
        }
        let message = Message::parse(reply.to_string().as_bytes(), 65536).unwrap();
        let result = request.finish(&ic, observed, &message, case != 4);
        if case == 0 {
            let Response::Transfer(proof) = result.unwrap() else {
                panic!("expected issuer evidence")
            };
            assert_eq!(proof.issuer(), f.local);
            assert_eq!(proof.association(), a);
            assert_eq!(proof.result().fee, 2);
        } else {
            assert!(result.is_err());
        }
    }
    let request = Request::balance(&c, auth.clone(), a).unwrap();
    let response = respond(
        &f,
        association(&f, f.recipient),
        Message::parse(request.bytes(), 65536)
            .unwrap()
            .value()
            .clone(),
    );
    assert!(matches!(
        request.finish(&ic, a, &response, true),
        Ok(Response::Balance(7))
    ));
    let pg = f.peer.grant(f.local, Action::Invoke, 16_777_216).unwrap();
    let wrong = f
        .authority
        .grant(f.local, ledger::authority::Action::Read, 16_777_216)
        .unwrap();
    let query = Message::parse(br#"{"type":"ledger.balance"}"#, 65536).unwrap();
    assert_eq!(
        read::respond(
            &context(&pg),
            f.peer.as_ref(),
            f.ledger.as_ref(),
            &ledger_context(&wrong),
            association(&f, f.owner),
            &query,
            Limits::default()
        ),
        Err(Error::Denied)
    );
}
