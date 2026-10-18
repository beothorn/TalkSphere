//! TR-05/TR-06 orchestration checks with strict, non-cryptographic primitive scripts.
use super::*;
use crate::modules::transport::{
    codec::FrameType,
    test_support::{
        crypto::{CryptoCall, CryptoReply, ScriptedCryptography, SecretLog, TestSecret},
        script::Script,
        vectors,
    },
};

// Fresh labels stand for connection-owned keys; fixture prefixes preserve role direction.
fn session(log: &SecretLog) -> Integrity<TestSecret> {
    Integrity::new(SessionKeys {
        send: DirectionKeys {
            key: log.issue(1),
            nonce_prefix: vectors::array("initiator-to-acceptor nonce prefix"),
        },
        receive: DirectionKeys {
            key: log.issue(2),
            nonce_prefix: vectors::array("acceptor-to-initiator nonce prefix"),
        },
    })
}
// Published PING/PONG bytes are supplied independently of sealing/verification code.
fn frame(occurrence: usize, tagged: bool) -> WireFrame {
    WireFrame::new(
        FrameHeader::decode(&vectors::bytes("header", occurrence)).unwrap(),
        vectors::bytes("payload", occurrence),
        tagged.then(|| {
            vectors::bytes("integrity tag", occurrence)
                .try_into()
                .unwrap()
        }),
    )
    .unwrap()
}
// The exact original header and payload form the AAD for each direction.
fn aad(occurrence: usize) -> Vec<u8> {
    [
        vectors::bytes("header", occurrence),
        vectors::bytes("payload", occurrence),
    ]
    .concat()
}

// Outbound sequence advances only after a complete write; receive progresses independently.
#[test]
fn exact_vector_inputs_and_independent_sequences() {
    let log = SecretLog::default();
    let mut state = session(&log);
    let script = Script::new([
        (
            CryptoCall::Tag {
                key: 1,
                nonce: vectors::bytes("nonce prefix || uint64_be(0)", 0)
                    .try_into()
                    .unwrap(),
                authenticated_data: aad(0),
            },
            CryptoReply::Bytes16(Ok(vectors::bytes("integrity tag", 0).try_into().unwrap())),
        ),
        (
            CryptoCall::VerifyTag {
                key: 2,
                nonce: vectors::bytes("nonce prefix || uint64_be(0)", 1)
                    .try_into()
                    .unwrap(),
                authenticated_data: aad(1),
                tag: vectors::bytes("integrity tag", 1).try_into().unwrap(),
            },
            CryptoReply::Verified(Ok(())),
        ),
        (
            CryptoCall::Tag {
                key: 1,
                nonce: nonce(&vectors::array("initiator-to-acceptor nonce prefix"), 1),
                authenticated_data: aad(0),
            },
            CryptoReply::Bytes16(Ok([9; 16])),
        ),
    ]);
    let crypto = ScriptedCryptography(script.clone());
    let sealed = state.seal(frame(0, false), &crypto).unwrap();
    assert_eq!(
        sealed.tag().unwrap().as_slice(),
        vectors::bytes("integrity tag", 0)
    );
    assert_eq!(state.send_sequence, 0);
    assert_eq!(
        state.seal(frame(0, false), &crypto).err(),
        Some(IntegrityError::Busy)
    );
    let verified = state.verify(frame(1, true), &crypto).unwrap();
    assert_eq!(verified.header().frame_type(), FrameType::Pong);
    assert_eq!(verified.payload(), vectors::bytes("payload", 1));
    assert_eq!(verified.into_payload(), vectors::bytes("payload", 1));
    assert_eq!(state.receive_sequence, 1);
    assert_eq!(state.send_sequence, 0);
    state.sent().unwrap();
    assert_eq!(state.send_sequence, 1);
    assert_eq!(
        state.seal(frame(0, false), &crypto).unwrap().tag(),
        Some(&[9; 16])
    );
    state.sent().unwrap();
    script.assert_finished();
}
// Replay at the next sequence fails and returns no verified bytes; both keys are released.
#[test]
fn replay_failure_is_terminal_without_advancing_receive_sequence() {
    let log = SecretLog::default();
    let mut state = session(&log);
    let calls = (0..2).map(|seq| {
        (
            CryptoCall::VerifyTag {
                key: 2,
                nonce: nonce(&vectors::array("acceptor-to-initiator nonce prefix"), seq),
                authenticated_data: aad(1),
                tag: vectors::bytes("integrity tag", 1).try_into().unwrap(),
            },
            CryptoReply::Verified(if seq == 0 {
                Ok(())
            } else {
                Err(DependencyError::VerificationFailed)
            }),
        )
    });
    let crypto = ScriptedCryptography(Script::new(calls));
    state.verify(frame(1, true), &crypto).unwrap();
    assert_eq!(
        state.verify(frame(1, true), &crypto).err(),
        Some(IntegrityError::Dependency(
            DependencyError::VerificationFailed
        ))
    );
    assert_eq!(state.receive_sequence, 1);
    assert!(state.is_closed());
    assert_eq!(log.released(), [1, 2]);
    assert_eq!(
        state.verify(frame(1, true), &crypto).err(),
        Some(IntegrityError::Closed)
    );
    assert_eq!(
        state.seal(frame(0, false), &crypto).err(),
        Some(IntegrityError::Closed)
    );
    assert_eq!(state.sent(), Err(IntegrityError::Closed));
    crypto.0.assert_finished();
}
// Missing/extra tags and out-of-order write completion cannot keep a usable session.
#[test]
fn invalid_tag_shapes_and_completion_order_release_keys() {
    for operation in 0..3 {
        let log = SecretLog::default();
        let mut state = session(&log);
        let crypto = ScriptedCryptography(Script::new([]));
        let result = match operation {
            0 => state.verify(frame(1, false), &crypto).err(),
            1 => state.seal(frame(0, true), &crypto).err(),
            _ => state.sent().err(),
        };
        assert_eq!(result, Some(IntegrityError::Shape));
        assert_eq!(log.released(), [1, 2]);
    }
}
// Neither counter can wrap; maximum receive is verified but never dispatched.
#[test]
fn sequence_exhaustion_closes_without_reusing_a_nonce() {
    let log = SecretLog::default();
    let mut state = session(&log);
    state.send_sequence = u64::MAX;
    assert_eq!(
        state
            .seal(frame(0, false), &ScriptedCryptography(Script::new([])))
            .err(),
        Some(IntegrityError::Exhausted)
    );
    let log = SecretLog::default();
    let mut state = session(&log);
    state.receive_sequence = u64::MAX;
    let crypto = ScriptedCryptography(Script::new([(
        CryptoCall::VerifyTag {
            key: 2,
            nonce: nonce(
                &vectors::array("acceptor-to-initiator nonce prefix"),
                u64::MAX,
            ),
            authenticated_data: aad(1),
            tag: vectors::bytes("integrity tag", 1).try_into().unwrap(),
        },
        CryptoReply::Verified(Ok(())),
    )]));
    assert_eq!(
        state.verify(frame(1, true), &crypto).err(),
        Some(IntegrityError::Exhausted)
    );
    assert!(state.is_closed());
    assert_eq!(state.receive_sequence, u64::MAX);
    assert_eq!(log.released(), [1, 2]);
}
// Wrong-direction/old-session tags and tag generation failures are delegated and never fabricated.
#[test]
fn primitive_rejection_uses_exact_connection_material_and_discards_session() {
    let log = SecretLog::default();
    let mut state = session(&log);
    let crypto = ScriptedCryptography(Script::new([(
        CryptoCall::Tag {
            key: 1,
            nonce: nonce(&vectors::array("initiator-to-acceptor nonce prefix"), 0),
            authenticated_data: aad(0),
        },
        CryptoReply::Bytes16(Err(DependencyError::Unavailable)),
    )]));
    assert_eq!(
        state.seal(frame(0, false), &crypto).err(),
        Some(IntegrityError::Dependency(DependencyError::Unavailable))
    );
    for tag in [
        vectors::bytes("integrity tag", 0).try_into().unwrap(),
        [99; 16],
    ] {
        let log = SecretLog::default();
        let mut state = session(&log);
        let wrong = WireFrame::new(
            FrameHeader::new(FrameType::Pong, 0, 8).unwrap(),
            vectors::bytes("payload", 1),
            Some(tag),
        )
        .unwrap();
        let crypto = ScriptedCryptography(Script::new([(
            CryptoCall::VerifyTag {
                key: 2,
                nonce: nonce(&vectors::array("acceptor-to-initiator nonce prefix"), 0),
                authenticated_data: aad(1),
                tag,
            },
            CryptoReply::Verified(Err(DependencyError::VerificationFailed)),
        )]));
        assert_eq!(
            state.verify(wrong, &crypto).err(),
            Some(IntegrityError::Dependency(
                DependencyError::VerificationFailed
            ))
        );
        assert!(state.is_closed());
    }
}
