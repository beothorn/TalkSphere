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

// Both roles reject bad signatures and canonical-profile negatives before agreement or HKDF, including zero ephemeral points.
#[test]
fn real_handshake_failure_precedes_agreement_for_both_roles() {
    use crate::modules::cryptography::{
        primitives::signature,
        secrets::{Secret, Signing},
    };
    use zeroize::Zeroizing;
    for role in [ConnectionRole::Initiator, ConnectionRole::Acceptor] {
        let peer = if role == ConnectionRole::Initiator {
            "acceptor"
        } else {
            "initiator"
        };
        for case in 0..6 {
            let mut h = Harness::pending(role, 3);
            let mut hello = vector(&format!("{peer} HELLO"));
            let mut auth = vector(&format!("{peer} AUTH payload"));
            match case {
                0 => auth[0] ^= 1,
                1 => {
                    hello[36..68].fill(0);
                    auth[0] ^= 1;
                }
                2 => {
                    hello[3..35].fill(0);
                    hello[3] = 1;
                    auth.fill(0);
                    auth[0] = 1;
                }
                3 => {
                    auth[..32].fill(0);
                    auth[0] = 1;
                }
                4 => {
                    auth[32..].copy_from_slice(&crate::modules::cryptography::test_support::hex(
                        "edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010",
                    ));
                }
                _ => {
                    hello[36..68].fill(0);
                    let local = vector(&format!("{} HELLO", role_name(role)));
                    let (i, a) = if role == ConnectionRole::Initiator {
                        (&local, &hello)
                    } else {
                        (&hello, &local)
                    };
                    let mut message = b"CR2SE-NETWORK-AUTH-V1\0".to_vec();
                    message.push(if role == ConnectionRole::Initiator {
                        2
                    } else {
                        1
                    });
                    for bytes in [i, a] {
                        message.extend((bytes.len() as u32).to_be_bytes());
                        message.extend(bytes);
                    }
                    let key = Secret::<Signing>::new(Zeroizing::new(
                        vector(&format!("{peer} Ed25519 seed")).try_into().unwrap(),
                    ));
                    auth = signature::sign(&key, &message, 4096).unwrap().to_vec();
                }
            }
            h.control.push_read(ReadStep::Bytes(
                [
                    aad(FrameType::Hello, 0, &hello),
                    aad(FrameType::Auth, 0, &auth),
                ]
                .concat(),
            ));
            for _ in 0..32 {
                if h.poll().is_ready() {
                    break;
                }
            }
            assert!(h.control.is_closed());
            assert!(h.connection.session().is_none());
            assert!(
                !h.events()
                    .iter()
                    .any(|e| matches!(e, Event::Authenticated(_) | Event::Data { .. }))
            );
            let calls = h.crypto.calls.lock().unwrap();
            assert!(!calls.contains(&"extract"));
            if case == 5 {
                assert!(calls.contains(&"verified"));
                assert!(calls.contains(&"agree"));
            } else {
                assert!(!calls.contains(&"agree"));
            }
        }
    }
}

// Malformed ordering/width/selectors and withheld proof never produce an authenticated session or agreement.
#[test]
fn malformed_and_timed_out_handshakes_do_not_publish_keys() {
    for role in [ConnectionRole::Initiator, ConnectionRole::Acceptor] {
        let peer = if role == ConnectionRole::Initiator {
            "acceptor"
        } else {
            "initiator"
        };
        for case in 0..9 {
            let mut h = Harness::pending(role, usize::MAX);
            let hello = vector(&format!("{peer} HELLO"));
            let bytes = match case {
                0 => aad(FrameType::Auth, 0, &vector(&format!("{peer} AUTH payload"))),
                1 => aad(FrameType::Hello, 0, &hello[..99]),
                2 => {
                    let mut changed = hello.clone();
                    changed.push(0);
                    aad(FrameType::Hello, 0, &changed)
                }
                3 => {
                    let mut changed = hello.clone();
                    changed[0] = 2;
                    aad(FrameType::Hello, 0, &changed)
                }
                4 => [
                    aad(FrameType::Hello, 0, &hello),
                    aad(FrameType::Hello, 0, &hello),
                ]
                .concat(),
                6 => [
                    aad(FrameType::Hello, 0, &hello),
                    aad(
                        FrameType::Auth,
                        0,
                        &vector(&format!("{peer} AUTH payload"))[..63],
                    ),
                ]
                .concat(),
                7 => {
                    let mut auth = vector(&format!("{peer} AUTH payload"));
                    auth.push(0);
                    [
                        aad(FrameType::Hello, 0, &hello),
                        aad(FrameType::Auth, 0, &auth),
                    ]
                    .concat()
                }
                8 => aad(FrameType::Data, 2, b"early"),
                _ => aad(FrameType::Hello, 0, &hello),
            };
            h.control.push_read(ReadStep::Bytes(bytes));
            for _ in 0..8 {
                let _ = h.poll();
            }
            if case == 5 {
                assert!(h.connection.session().is_none());
                h.now += std::time::Duration::from_secs(4);
                let _ = h.poll();
            }
            assert!(h.control.is_closed());
            assert!(h.connection.session().is_none());
            assert!(!h.crypto.calls.lock().unwrap().contains(&"agree"));
            assert!(
                !h.events()
                    .iter()
                    .any(|event| matches!(event, Event::Authenticated(_)))
            );
        }
    }
}

// Partial local AUTH output and wrong pinned identity both fail without raw response or session derivation.
#[test]
fn partial_auth_write_and_expected_identity_failure_do_not_derive() {
    use crate::modules::transport::test_support::socket::WriteStep;
    for role in [ConnectionRole::Initiator, ConnectionRole::Acceptor] {
        let peer = if role == ConnectionRole::Initiator {
            "acceptor"
        } else {
            "initiator"
        };
        let mut h = Harness::pending(role, usize::MAX);
        h.control.replace_writes([
            WriteStep::Accept(112),
            WriteStep::Accept(3),
            WriteStep::Error(std::io::ErrorKind::BrokenPipe),
        ]);
        h.control.push_read(ReadStep::Bytes(aad(
            FrameType::Hello,
            0,
            &vector(&format!("{peer} HELLO")),
        )));
        for _ in 0..32 {
            if h.poll().is_ready() {
                break;
            }
        }
        assert!(h.control.is_closed());
        assert_eq!(h.control.written().len(), 115);
        assert!(!h.crypto.calls.lock().unwrap().contains(&"agree"));
        assert!(h.connection.session().is_none());
        let mut h = Harness::configured(
            role,
            usize::MAX,
            ExpectedPeer::Known(crate::modules::transport::ports::identity::IdentityId(
                [0; 32],
            )),
        );
        h.control.push_read(ReadStep::Bytes(aad(
            FrameType::Hello,
            0,
            &vector(&format!("{peer} HELLO")),
        )));
        for _ in 0..32 {
            if h.poll().is_ready() {
                break;
            }
        }
        assert!(h.control.is_closed());
        assert!(!h.crypto.calls.lock().unwrap().contains(&"agree"));
        assert!(h.connection.session().is_none());
    }
}

// Actual AEAD failures close silently before OPEN or DATA dispatch, including replay and wrong directional material.
#[test]
fn integrity_mutation_replay_and_direction_confusion_close_silently() {
    use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::AeadInPlace};
    let open = Open::new("cr2se.peer", 1).unwrap().encode();
    for case in 0..6 {
        let mut h = Harness::new();
        let mut bytes = h.expect_receive(FrameType::Open, 2, &open, true);
        match case {
            0 => bytes[7] ^= 4,
            1 => bytes[12] ^= 1,
            2 => *bytes.last_mut().unwrap() ^= 1,
            3 => {
                h.control.push_read(ReadStep::Bytes(bytes.clone()));
                assert!(h.poll().is_pending());
                assert!(matches!(h.events()[0], Event::StreamOpened { .. }));
            }
            4 => bytes = protected(FrameType::Open, 2, &open, 0, "initiator-to-acceptor"),
            _ => {
                let body = aad(FrameType::Open, 2, &open);
                let mut nonce = vector("initiator-to-acceptor nonce prefix");
                nonce.extend(0u64.to_be_bytes());
                let cipher =
                    XChaCha20Poly1305::new_from_slice(&vector("acceptor-to-initiator key"))
                        .unwrap();
                let tag = cipher
                    .encrypt_in_place_detached(XNonce::from_slice(&nonce), &body, &mut [])
                    .unwrap();
                bytes = [body, tag.to_vec()].concat();
            }
        }
        let before = h.control.written().len();
        h.control.push_read(ReadStep::Bytes(bytes));
        assert!(h.poll().is_ready());
        assert!(h.control.is_closed());
        assert_eq!(h.control.written().len(), before);
        assert!(
            !h.events()
                .iter()
                .any(|event| matches!(event, Event::StreamOpened { .. } | Event::Data { .. }))
        );
    }
}

// A changed signed HELLO nonce installs different session keys, so a prior connection's valid tag cannot authenticate.
#[test]
fn old_connection_tag_is_rejected_under_new_signed_transcript() {
    use crate::modules::cryptography::{
        primitives::signature,
        secrets::{Secret, Signing},
    };
    let mut h = Harness::pending(ConnectionRole::Initiator, usize::MAX);
    let local = vector("initiator HELLO");
    let mut remote = vector("acceptor HELLO");
    remote[68] ^= 1;
    let mut message = b"CR2SE-NETWORK-AUTH-V1\0".to_vec();
    message.push(2);
    for hello in [&local, &remote] {
        message.extend((hello.len() as u32).to_be_bytes());
        message.extend(hello);
    }
    let key = Secret::<Signing>::new(zeroize::Zeroizing::new(
        vector("acceptor Ed25519 seed").try_into().unwrap(),
    ));
    let proof = signature::sign(&key, &message, 4096).unwrap();
    h.control.push_read(ReadStep::Bytes(
        [
            aad(FrameType::Hello, 0, &remote),
            aad(FrameType::Auth, 0, &proof),
        ]
        .concat(),
    ));
    for _ in 0..16 {
        let _ = h.poll();
    }
    assert!(h.connection.session().is_some());
    assert!(matches!(h.events()[0], Event::Authenticated(_)));
    let before = h.control.written().len();
    let bytes = protected(
        FrameType::Open,
        2,
        &Open::new("cr2se.peer", 1).unwrap().encode(),
        0,
        "acceptor-to-initiator",
    );
    h.control.push_read(ReadStep::Bytes(bytes));
    assert!(h.poll().is_ready());
    assert_eq!(h.control.written().len(), before);
    assert!(h.control.is_closed());
    assert!(
        !h.events()
            .iter()
            .any(|event| matches!(event, Event::StreamOpened { .. } | Event::Data { .. }))
    );
}

// Acquired native key owners are all released when an authenticated connection fails integrity and its identity drops.
#[test]
fn native_connection_failure_releases_acquired_secret_owners() {
    use crate::modules::cryptography::test_support::observe_releases;
    let (_, released) = observe_releases(|| {
        let mut h = Harness::new();
        let bytes = h.expect_receive(
            FrameType::Open,
            2,
            &Open::new("cr2se.peer", 1).unwrap().encode(),
            false,
        );
        h.control.push_read(ReadStep::Bytes(bytes));
        assert!(h.poll().is_ready());
        drop(h);
    });
    // Four credential staging owners, two loaded identity owners and five ephemeral/derived owners were acquired.
    assert_eq!(released.load(std::sync::atomic::Ordering::SeqCst), 11);
}

// Exact local and remote allocation retain parity and consume malformed/refused OPEN IDs under valid real tags.
#[test]
fn native_stream_allocation_and_protocol_selection_are_exact() {
    use crate::modules::transport::codec::control::{ErrorCode, ErrorPayload};
    let open = Open::new("cr2se.peer", 1).unwrap().encode();
    let mut h = Harness::new();
    for id in [1, 3, 5] {
        let stream = h
            .connection
            .session()
            .unwrap()
            .open(&h.registry, &h.owner)
            .unwrap();
        assert_eq!(stream.wire_id().get(), id);
        let expected = h.expect_send(FrameType::Open, id, &open);
        let start = h.control.written().len();
        let _ = h.poll();
        assert_eq!(&h.control.written()[start..], expected);
    }
    for id in [2, 4, 6] {
        h.receive(FrameType::Open, id, open.clone(), None);
        assert!(h.events().iter().any(
            |event| matches!(event,Event::StreamOpened{stream,..} if stream.wire_id().get()==id)
        ));
    }
    let mut trailing = open.clone();
    trailing.push(0);
    let mut invalid_utf8 = open.clone();
    invalid_utf8[2] = 255;
    let mut zero_version = open.clone();
    let len = zero_version.len();
    zero_version[len - 4..].fill(0);
    for (payload, code) in [
        (vec![], ErrorCode::ProtocolError),
        (trailing, ErrorCode::ProtocolError),
        (invalid_utf8, ErrorCode::ProtocolError),
        (zero_version, ErrorCode::ProtocolError),
        (
            Open::new("unknown", 1).unwrap().encode(),
            ErrorCode::UnsupportedProtocol,
        ),
        (
            Open::new("cr2se.peer", 2).unwrap().encode(),
            ErrorCode::UnsupportedProtocol,
        ),
    ] {
        let mut h = Harness::new();
        h.receive(
            FrameType::Open,
            2,
            payload,
            Some((
                FrameType::Error,
                2,
                ErrorPayload::new(code, "").unwrap().encode(),
            )),
        );
        h.receive(FrameType::Open, 4, open.clone(), None);
        assert!(!h.control.is_closed());
        assert!(
            h.events()
                .iter()
                .any(|e| matches!(e,Event::StreamOpened{stream,..} if stream.wire_id().get()==4))
        );
    }
}

// Wrong parity, skipped IDs, reuse and data for never-created streams remain connection failures after native integrity succeeds.
#[test]
fn native_stream_id_violations_close_connection() {
    use crate::modules::transport::codec::control::{ErrorCode, ErrorPayload};
    for (kind, id, reuse) in [
        (FrameType::Open, 1, false),
        (FrameType::Open, 4, false),
        (FrameType::Open, 2, true),
        (FrameType::Data, 2, false),
    ] {
        let mut h = Harness::new();
        let open = Open::new("cr2se.peer", 1).unwrap().encode();
        if reuse {
            h.receive(FrameType::Open, 2, open.clone(), None);
            h.events();
        }
        h.receive(
            kind,
            id,
            if kind == FrameType::Open {
                open
            } else {
                vec![1]
            },
            Some((
                FrameType::Error,
                0,
                ErrorPayload::new(ErrorCode::ProtocolError, "")
                    .unwrap()
                    .encode(),
            )),
        );
        assert!(h.control.is_closed());
    }
}

use crate::modules::transport::{
    codec::control::{ErrorCode, ErrorPayload},
    config::MAX_PAYLOAD_BYTES,
};
// Encode fixed diagnostic-free errors for independent native response assertions.
fn error(code: ErrorCode) -> Vec<u8> {
    ErrorPayload::new(code, "").unwrap().encode()
}

// Repeat the complete terminal/half-close scenario with persisted Identity and native tags.
#[test]
fn routing_halves_cancellation_and_reuse_through_socket() {
    let mut h = Harness::new();
    h.receive(
        FrameType::Open,
        2,
        vec![],
        Some((FrameType::Error, 2, error(ErrorCode::ProtocolError))),
    );
    h.receive(
        FrameType::Open,
        4,
        Open::new("unknown", 1).unwrap().encode(),
        Some((FrameType::Error, 4, error(ErrorCode::UnsupportedProtocol))),
    );
    h.receive(
        FrameType::Open,
        6,
        Open::new("cr2se.peer", 1).unwrap().encode(),
        None,
    );
    let stream = match h.events().pop().unwrap() {
        Event::StreamOpened { stream, .. } => stream,
        event => panic!("unexpected {event:?}"),
    };
    h.receive(FrameType::Data, 6, vec![1; MAX_PAYLOAD_BYTES], None);
    assert!(
        matches!(h.events().pop(), Some(Event::Data { bytes, .. }) if bytes.len() == MAX_PAYLOAD_BYTES)
    );
    h.receive(FrameType::End, 6, vec![], None);
    h.connection
        .session()
        .unwrap()
        .send(&h.owner, &stream, b"reply".to_vec())
        .unwrap();
    h.expect_send(FrameType::Data, 6, b"reply");
    assert!(h.poll().is_pending());
    h.connection
        .session()
        .unwrap()
        .end(&h.owner, &stream)
        .unwrap();
    h.expect_send(FrameType::End, 6, &[]);
    assert!(h.poll().is_pending());
    assert!(h.events().iter().any(|e| matches!(
        e,
        Event::StreamClosed {
            outcome: StreamOutcome::Completed,
            ..
        }
    )));
    h.receive(
        FrameType::Open,
        8,
        Open::new("cr2se.peer", 1).unwrap().encode(),
        None,
    );
    h.receive(FrameType::Cancel, 8, error(ErrorCode::Cancelled), None);
    h.receive(FrameType::Data, 8, vec![1], None);
    h.receive(FrameType::End, 8, vec![99], None);
    assert_eq!(
        h.events()
            .iter()
            .filter(|e| matches!(e, Event::StreamClosed { .. }))
            .count(),
        1
    );
    h.receive(
        FrameType::Open,
        8,
        vec![],
        Some((FrameType::Error, 0, error(ErrorCode::ProtocolError))),
    );
    assert!(h.control.is_closed());
}

use crate::modules::transport::codec::control::GoAway;
use std::time::Duration;

// Native-tag repetition verifies probe/drain state behavior independently of scripted crypto replies.
#[test]
fn ping_matching_timeout_and_graceful_empty_drain() {
    let mut h = Harness::new();
    h.receive(
        FrameType::Ping,
        0,
        vec![1; 8],
        Some((FrameType::Pong, 0, vec![1; 8])),
    );
    h.receive(FrameType::Pong, 0, vec![2; 8], None);
    h.connection
        .session()
        .unwrap()
        .ping([3; 8], Duration::from_secs(1), h.now)
        .unwrap();
    h.expect_send(FrameType::Ping, 0, &[3; 8]);
    assert!(h.poll().is_pending());
    h.receive(FrameType::Pong, 0, vec![4; 8], None);
    h.now += Duration::from_secs(1);
    assert!(h.poll().is_pending());
    assert!(h.events().iter().any(|e| matches!(
        e,
        Event::PingFinished {
            result: Err(Failure {
                code: FailureCode::DeadlineExceeded,
                ..
            }),
            ..
        }
    )));
    h.connection
        .session()
        .unwrap()
        .drain(ShutdownReason::Normal)
        .unwrap();
    h.expect_send(
        FrameType::GoAway,
        0,
        &GoAway::new(0, ErrorCode::NoError, "").unwrap().encode(),
    );
    assert!(h.poll().is_ready());
    assert!(
        h.events()
            .iter()
            .any(|e| matches!(e, Event::ConnectionClosed { failure: None, .. }))
    );
}

// Native-tag repetition verifies probe/drain state behavior independently of scripted crypto replies.
#[test]
fn goaway_refusal_and_accepted_stream_completion() {
    let mut h = Harness::new();
    let mut streams = vec![];
    for id in [1, 3, 5] {
        streams.push(
            h.connection
                .session()
                .unwrap()
                .open(&h.registry, &h.owner)
                .unwrap(),
        );
        h.expect_send(
            FrameType::Open,
            id,
            &Open::new("cr2se.peer", 1).unwrap().encode(),
        );
    }
    assert!(h.poll().is_pending());
    h.events();
    h.receive(
        FrameType::GoAway,
        0,
        GoAway::new(3, ErrorCode::NoError, "").unwrap().encode(),
        Some((
            FrameType::GoAway,
            0,
            GoAway::new(0, ErrorCode::NoError, "").unwrap().encode(),
        )),
    );
    assert!(h.events().iter().any(|e| matches!(e, Event::StreamClosed { stream, outcome: StreamOutcome::Failed(Failure { code: FailureCode::Refused, .. }) } if stream.wire_id().get() == 5)));
    h.receive(
        FrameType::Open,
        2,
        vec![],
        Some((FrameType::Error, 2, error(ErrorCode::RefusedStream))),
    );
    h.receive(
        FrameType::GoAway,
        0,
        GoAway::new(1, ErrorCode::NoError, "").unwrap().encode(),
        None,
    );
    h.events();
    h.receive(FrameType::End, 1, vec![], None);
    h.connection
        .session()
        .unwrap()
        .end(&h.owner, &streams[0])
        .unwrap();
    h.expect_send(FrameType::End, 1, &[]);
    assert!(h.poll().is_ready());
    assert!(h.events().iter().any(|e| matches!(
        e,
        Event::StreamClosed {
            outcome: StreamOutcome::Completed,
            ..
        }
    )));
}

// Native-tag repetition verifies probe/drain state behavior independently of scripted crypto replies.
#[test]
fn protected_state_violations_close_connection() {
    for (kind, id, payload) in [
        (
            FrameType::Open,
            4,
            Open::new("cr2se.peer", 1).unwrap().encode(),
        ),
        (FrameType::Data, 2, vec![1]),
        (
            FrameType::GoAway,
            0,
            GoAway::new(2, ErrorCode::NoError, "").unwrap().encode(),
        ),
        (
            FrameType::GoAway,
            0,
            GoAway::new(1, ErrorCode::NoError, "").unwrap().encode(),
        ),
    ] {
        let mut h = Harness::new();
        h.receive(
            kind,
            id,
            payload,
            Some((FrameType::Error, 0, error(ErrorCode::ProtocolError))),
        );
        assert!(h.control.is_closed());
    }
}

// Native-tag repetition verifies probe/drain state behavior independently of scripted crypto replies.
#[test]
fn eof_after_completed_goaway_is_normal() {
    let mut h = Harness::new();
    h.connection
        .session()
        .unwrap()
        .drain(ShutdownReason::Normal)
        .unwrap();
    h.expect_send(
        FrameType::GoAway,
        0,
        &GoAway::new(0, ErrorCode::NoError, "").unwrap().encode(),
    );
    h.control.push_read(ReadStep::Eof);
    assert!(h.poll().is_ready());
    assert!(
        h.events()
            .iter()
            .any(|e| matches!(e, Event::ConnectionClosed { failure: None, .. }))
    );
}

// Native-tag repetition verifies probe/drain state behavior independently of scripted crypto replies.
#[test]
fn peer_drain_reason_is_preserved() {
    let mut h = Harness::new();
    h.receive(
        FrameType::GoAway,
        0,
        GoAway::new(0, ErrorCode::IdExhausted, "").unwrap().encode(),
        Some((
            FrameType::GoAway,
            0,
            GoAway::new(0, ErrorCode::NoError, "").unwrap().encode(),
        )),
    );
    assert!(h.control.is_closed());
    assert!(h.events().iter().any(|e| matches!(
        e,
        Event::ConnectionClosed {
            failure: Some(Failure {
                origin: FailureOrigin::Peer,
                code: FailureCode::IdExhausted
            }),
            ..
        }
    )));
}

use crate::modules::transport::codec::FrameHeader;

// Correct native tags reach the actual bound, control or truncation state rule under test.
#[test]
fn malformed_header_response_matrix() {
    for case in 0..6 {
        let mut h = Harness::new();
        let mut header = FrameHeader::new(FrameType::Data, 2, 1).unwrap().encode();
        match case {
            0 => header[0] = 2,
            1 => header[2] = 1,
            2 => header[1] = 255,
            3 => header[8..12].copy_from_slice(&65537u32.to_be_bytes()),
            4 => header[4..8].fill(0),
            _ => header[1] = FrameType::Ping as u8,
        }
        let start = h.control.written().len();
        if case != 0 {
            h.expect_send(
                FrameType::Error,
                0,
                &error(if case == 3 {
                    ErrorCode::FrameSizeError
                } else {
                    ErrorCode::ProtocolError
                }),
            );
        }
        h.control.push_read(ReadStep::Bytes(header.to_vec()));
        assert!(h.poll().is_ready());
        assert!(h.control.is_closed());
        if case == 0 {
            assert_eq!(h.control.written().len(), start);
        }
        assert!(!h.events().iter().any(|e| matches!(e, Event::Data { .. })));
    }
}

// Correct native tags reach the actual bound, control or truncation state rule under test.
#[test]
fn malformed_protected_controls_are_connection_errors() {
    for (kind, payload) in [
        (FrameType::Ping, vec![0; 7]),
        (FrameType::Error, vec![0, 0, 0, 0]),
        (FrameType::Error, vec![255, 255, 0, 0]),
        (FrameType::Error, vec![0, 1, 0, 1, 255]),
        (FrameType::GoAway, vec![0; 7]),
    ] {
        let mut h = Harness::new();
        h.receive(
            kind,
            0,
            payload,
            Some((FrameType::Error, 0, error(ErrorCode::ProtocolError))),
        );
        assert!(h.control.is_closed());
    }
}

// Correct native tags reach the actual bound, control or truncation state rule under test.
#[test]
fn every_incomplete_protected_prefix_is_truncation() {
    let bytes = protected(FrameType::Data, 2, b"abc", 1, "acceptor-to-initiator");
    for end in 1..bytes.len() {
        let mut h = Harness::new();
        h.receive(
            FrameType::Open,
            2,
            Open::new("cr2se.peer", 1).unwrap().encode(),
            None,
        );
        h.events();
        h.control.push_read(ReadStep::Bytes(bytes[..end].to_vec()));
        h.control.push_read(ReadStep::Eof);
        assert!(h.poll().is_ready());
        let events = h.events();
        assert!(!events.iter().any(|e| matches!(e, Event::Data { .. })));
        assert!(events.iter().any(|e| matches!(
            e,
            Event::StreamClosed {
                outcome: StreamOutcome::Failed(Failure {
                    code: FailureCode::TruncatedFrame,
                    ..
                }),
                ..
            }
        )));
    }
}

// Correct native tags reach the actual bound, control or truncation state rule under test.
#[test]
fn bounded_slow_consumer_does_not_stop_another_stream() {
    let mut h = Harness::new();
    for id in [2, 4] {
        h.receive(
            FrameType::Open,
            id,
            Open::new("cr2se.peer", 1).unwrap().encode(),
            None,
        );
    }
    h.events();
    h.receive(FrameType::Data, 2, vec![0; MAX_PAYLOAD_BYTES], None);
    h.receive(
        FrameType::Data,
        2,
        vec![1],
        Some((FrameType::Error, 2, error(ErrorCode::ResourceLimit))),
    );
    h.receive(FrameType::Data, 4, vec![9], None);
    let events = h.events();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::StreamClosed {
            outcome: StreamOutcome::Failed(Failure {
                code: FailureCode::ResourceLimit,
                ..
            }),
            ..
        }
    )));
    assert!(events.iter().any(|e| matches!(e, Event::Data { stream, bytes } if stream.wire_id().get() == 4 && *bytes == [9])));
    assert!(!h.control.is_closed());
}

// Every literal diagnostic row preserves accepted bytes and rejects malformed text before trusted delivery under real tags.
#[test]
fn native_diagnostic_reason_vectors_reach_control_validation() {
    use crate::modules::cryptography::test_support::hex;
    for line in crate::modules::identity::test_support::TRANSPORT
        .lines()
        .filter(|line| line.starts_with("| empty |") || line.starts_with("| `"))
    {
        let columns: Vec<_> = line.split('|').map(str::trim).collect();
        if columns.len() != 6 {
            continue;
        }
        let reason_bytes = if columns[1] == "empty" {
            vec![]
        } else {
            hex(columns[1].trim_matches('`'))
        };
        let error_bytes = hex(columns[2].trim_matches('`'));
        let goaway_bytes = hex(columns[3].trim_matches('`'));
        let valid = columns[4].starts_with("accept");
        let reason = std::str::from_utf8(&reason_bytes).unwrap();
        if valid {
            assert_eq!(
                ErrorPayload::new(ErrorCode::ProtocolError, reason)
                    .unwrap()
                    .encode(),
                error_bytes
            );
            assert_eq!(
                GoAway::new(0, ErrorCode::ProtocolError, reason)
                    .unwrap()
                    .encode(),
                goaway_bytes
            );
        } else {
            assert!(ErrorPayload::new(ErrorCode::ProtocolError, reason).is_err());
            assert!(GoAway::new(0, ErrorCode::ProtocolError, reason).is_err());
        }
        for kind in [FrameType::Error, FrameType::Cancel, FrameType::GoAway] {
            let mut h = Harness::new();
            let id = if kind == FrameType::Cancel {
                h.receive(
                    FrameType::Open,
                    2,
                    Open::new("cr2se.peer", 1).unwrap().encode(),
                    None,
                );
                h.events();
                2
            } else {
                0
            };
            let payload = if kind == FrameType::GoAway {
                goaway_bytes.clone()
            } else {
                error_bytes.clone()
            };
            let response = if !valid {
                Some((FrameType::Error, 0, error(ErrorCode::ProtocolError)))
            } else if kind == FrameType::GoAway {
                Some((
                    FrameType::GoAway,
                    0,
                    GoAway::new(0, ErrorCode::NoError, "").unwrap().encode(),
                ))
            } else {
                None
            };
            h.receive(kind, id, payload, response);
            assert_eq!(h.control.is_closed(), !valid || kind != FrameType::Cancel);
        }
    }
    let accepted = format!("{}\u{feff}", "a".repeat(1021));
    assert_eq!(accepted.len(), 1024);
    assert!(ErrorPayload::new(ErrorCode::ProtocolError, &accepted).is_ok());
    let rejected = format!("{}\u{feff}", "a".repeat(1022));
    assert_eq!(rejected.len(), 1025);
    assert!(ErrorPayload::new(ErrorCode::ProtocolError, &rejected).is_err());
}

#[path = "privacy.rs"]
mod privacy;
