//! Actual Board publication and Peer Protocol metadata response composition.
use super::{
    exchange::{
        Direction::{Local, Remote},
        Exchange, Outcome,
    },
    message::{Kind, Message},
    metadata,
    record::Record,
    test_support::board::{DEFINITION, context, published},
};
use crate::modules::board::{self, authority::Action};
// Available metadata returns complete bounded controls, original definition bytes and exact large document numbers with both directional ENDs.
#[test]
fn actual_board_metadata() {
    let (owner, a, id) = published();
    let read = a.grant(id, Action::Read, 1_000_000).unwrap();
    let c = context(&read);
    for (raw, expected) in [
        (br#"{"type":"board.get"}"#.as_slice(), Kind::Board),
        (
            br#"{"type":"service.get","offeringId":"fixture"}"#,
            Kind::Definition,
        ),
    ] {
        let request = Message::parse(raw, 65536).unwrap();
        let response = metadata::respond(&owner, &c, &request).unwrap();
        let Record::Control(bytes) = response else {
            panic!("metadata is control-only");
        };
        let response = Message::parse(&bytes, 65536).unwrap();
        assert_eq!(response.kind(), expected);
        if expected == Kind::Board {
            assert_eq!(
                response.value()["board"]["unknown"].to_string(),
                "18446744073709551615"
            );
        } else {
            assert!(bytes.windows(DEFINITION.len()).any(|w| w == DEFINITION));
        }
        let mut x = Exchange::new(Local);
        x.control(Local, request.kind()).unwrap();
        x.end(Local).unwrap();
        x.control(Remote, response.kind()).unwrap();
        x.end(Remote).unwrap();
        assert_eq!(x.close(true), Ok(Outcome::Complete));
    }
    let missing =
        Message::parse(br#"{"type":"service.get","offeringId":"missing"}"#, 65536).unwrap();
    let Record::Control(bytes) = metadata::respond(&owner, &c, &missing).unwrap() else {
        panic!("control failure");
    };
    let response = Message::parse(&bytes, 65536).unwrap();
    assert_eq!(response.kind(), Kind::Failure);
    assert_eq!(response.value()["code"], "not_available");
    let denied = a.grant(id, Action::Publish, 1_000_000).unwrap();
    assert!(matches!(
        metadata::respond(&owner, &context(&denied), &missing),
        Err(board::Error::Denied)
    ));
}
