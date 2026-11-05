//! Real protected framing under controlled read segmentation and short writes.
#[path = "real_wire/support.rs"]
mod support;
use crate::modules::transport::{
    api::*,
    codec::{FrameType, open::Open},
    test_support::socket::ReadStep,
};
use support::*;

// Independent tag construction matches the literal upstream first-frame tag, not a local round-trip oracle.
#[test]
fn independent_tag_oracle_matches_pinned_first_frame() {
    let bytes = protected(
        FrameType::Ping,
        0,
        &vector("payload"),
        0,
        "initiator-to-acceptor",
    );
    assert_eq!(&bytes[bytes.len() - 16..], vector("integrity tag"));
}
// Every protected OPEN/DATA split preserves order and scoped stream ownership with native authentication.
#[test]
fn every_protected_split_preserves_real_authenticated_bytes() {
    let open = Open::new("cr2se.peer", 1).unwrap().encode();
    let length = open.len() + 28 + 3 + 28;
    for split in 0..=length {
        let mut h = Harness::new();
        let mut bytes = h.expect_receive(FrameType::Open, 2, &open, true);
        bytes.extend(h.expect_receive(FrameType::Data, 2, b"abc", true));
        for part in [&bytes[..split], &bytes[split..]] {
            if !part.is_empty() {
                h.control.push_read(ReadStep::Bytes(part.to_vec()));
            }
        }
        assert!(h.poll().is_pending());
        let events = h.events();
        assert!(
            matches!(&events[0],Event::StreamOpened{stream,..} if stream.authorize(stream.connection_id(),&h.owner).is_ok())
        );
        assert!(matches!(&events[1],Event::Data{bytes,..} if bytes==b"abc"));
        let expected = h.expect_send(FrameType::Pong, 0, &vector("payload"));
        let incoming = h.expect_receive(FrameType::Ping, 0, &vector("payload"), true);
        let start = h.control.written().len();
        h.control.push_read(ReadStep::Bytes(incoming));
        let _ = h.poll();
        assert_eq!(&h.control.written()[start..], expected);
    }
}
// One-byte reads and one-to-three-byte writes preserve exact pinned handshake output and coalesced protected dispatch.
#[test]
fn segmented_handshake_and_tag_transition_match_vectors() {
    for write_limit in [1, 2, 3] {
        let mut h = Harness::pending(ConnectionRole::Initiator, write_limit);
        let mut incoming = [
            aad(FrameType::Hello, 0, &vector("acceptor HELLO")),
            aad(FrameType::Auth, 0, &vector("acceptor AUTH payload")),
        ]
        .concat();
        incoming.extend(h.expect_receive(
            FrameType::Open,
            2,
            &Open::new("cr2se.peer", 1).unwrap().encode(),
            true,
        ));
        incoming.extend(h.expect_receive(FrameType::Data, 2, b"abc", true));
        for byte in incoming {
            h.control.push_read(ReadStep::Bytes(vec![byte]));
        }
        for _ in 0..32 {
            let _ = h.poll();
        }
        let events = h.events();
        assert!(matches!(events[0], Event::Authenticated(_)));
        assert!(matches!(events[1], Event::StreamOpened { .. }));
        assert!(matches!(&events[2],Event::Data{bytes,..} if bytes==b"abc"));
        assert_eq!(
            h.control.written(),
            [
                aad(FrameType::Hello, 0, &vector("initiator HELLO")),
                aad(FrameType::Auth, 0, &vector("initiator AUTH payload"))
            ]
            .concat()
        );
    }
}
