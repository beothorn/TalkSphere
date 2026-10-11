use super::*;
use crate::modules::transport::test_support::vectors;

#[test]
fn every_primitive_records_exact_arguments_and_returns_its_scripted_result() {
    let log = SecretLog::default();
    let public_key = vectors::array("initiator Ed25519 public key");
    let remote_key = vectors::array("acceptor ephemeral X25519 public key");
    let signature = vectors::array("initiator AUTH payload");
    let nonce32 = vectors::array("initiator HELLO nonce");
    let salt = vectors::array("HKDF salt");
    let prefix = vectors::array("initiator-to-acceptor nonce prefix");
    let nonce = vectors::array("nonce prefix || uint64_be(0)");
    let tag = vectors::array("integrity tag");
    let aad = [vectors::bytes("header", 0), vectors::bytes("payload", 0)].concat();
    // Inputs/outputs are scripts only, not a recomputation of the upstream vector.
    let calls = vec![
        CryptoCall::RandomNonce,
        CryptoCall::EphemeralKey,
        CryptoCall::Sha256(b"test hash input".to_vec()),
        CryptoCall::VerifySignature {
            public_key,
            message: b"test signing input".to_vec(),
            signature,
        },
        CryptoCall::Agree {
            private_key: 1,
            remote_key,
        },
        CryptoCall::Extract { salt, shared: 2 },
        CryptoCall::ExpandKey {
            key: 3,
            info: b"test key info".to_vec(),
        },
        CryptoCall::ExpandNoncePrefix {
            key: 3,
            info: b"test nonce info".to_vec(),
        },
        CryptoCall::Tag {
            key: 4,
            nonce,
            authenticated_data: aad.clone(),
        },
        CryptoCall::VerifyTag {
            key: 4,
            nonce,
            authenticated_data: aad.clone(),
            tag,
        },
    ];
    let script = Script::new(calls.iter().cloned().zip([
        CryptoReply::Bytes32(Ok(nonce32)),
        CryptoReply::EphemeralKey(Ok(EphemeralKey {
            public_key: vectors::array("initiator ephemeral X25519 public key"),
            private_key: log.issue(1),
        })),
        CryptoReply::Bytes32(Ok([9; 32])),
        CryptoReply::Verified(Ok(())),
        CryptoReply::Secret(Ok(log.issue(2))),
        CryptoReply::Secret(Ok(log.issue(3))),
        CryptoReply::Secret(Ok(log.issue(4))),
        CryptoReply::Bytes16(Ok(prefix)),
        CryptoReply::Bytes16(Ok(tag)),
        CryptoReply::Verified(Ok(())),
    ]));
    let mut crypto = ScriptedCryptography(script.clone());
    assert_eq!(crypto.random_nonce(), Ok(nonce32));
    let ephemeral = crypto.ephemeral_key().unwrap();
    assert_eq!(
        ephemeral.public_key,
        vectors::array("initiator ephemeral X25519 public key")
    );
    assert_eq!(crypto.sha256(b"test hash input"), Ok([9; 32]));
    assert_eq!(
        crypto.verify_signature(&public_key, b"test signing input", &signature),
        Ok(())
    );
    let shared = crypto.agree(ephemeral.private_key, &remote_key).unwrap();
    assert_eq!(shared.label(), 2);
    assert_eq!(log.released(), [1]);
    let prk = crypto.extract(&salt, shared).unwrap();
    assert_eq!(prk.label(), 3);
    assert_eq!(log.released(), [1, 2]);
    let key = crypto.expand_key(&prk, b"test key info").unwrap();
    assert_eq!(key.label(), 4);
    assert_eq!(
        crypto.expand_nonce_prefix(&prk, b"test nonce info"),
        Ok(prefix)
    );
    assert_eq!(crypto.tag(&key, &nonce, &aad), Ok(tag));
    assert_eq!(crypto.verify_tag(&key, &nonce, &aad, &tag), Ok(()));
    assert_eq!(script.calls(), calls);
    script.assert_finished();
    drop(key);
    drop(prk);
    assert_eq!(log.released(), [1, 2, 4, 3]);
}

#[test]
fn signature_agreement_and_tag_failures_are_preserved() {
    let log = SecretLog::default();
    let script = Script::new([
        (
            CryptoCall::VerifySignature {
                public_key: [1; 32],
                message: vec![0, 255],
                signature: [2; 64],
            },
            CryptoReply::Verified(Err(DependencyError::VerificationFailed)),
        ),
        (
            CryptoCall::Agree {
                private_key: 1,
                remote_key: [0; 32],
            },
            CryptoReply::Secret(Err(DependencyError::VerificationFailed)),
        ),
        (
            CryptoCall::VerifyTag {
                key: 2,
                nonce: [3; 24],
                authenticated_data: vec![4, 0, 5],
                tag: [6; 16],
            },
            CryptoReply::Verified(Err(DependencyError::VerificationFailed)),
        ),
        (
            CryptoCall::RandomNonce,
            CryptoReply::Bytes32(Err(DependencyError::Unavailable)),
        ),
    ]);
    let mut crypto = ScriptedCryptography(script.clone());
    assert_eq!(
        crypto.verify_signature(&[1; 32], &[0, 255], &[2; 64]),
        Err(DependencyError::VerificationFailed)
    );
    assert!(matches!(
        crypto.agree(log.issue(1), &[0; 32]),
        Err(DependencyError::VerificationFailed)
    ));
    assert_eq!(log.released(), [1]);
    let key = log.issue(2);
    assert_eq!(
        crypto.verify_tag(&key, &[3; 24], &[4, 0, 5], &[6; 16]),
        Err(DependencyError::VerificationFailed)
    );
    assert_eq!(crypto.random_nonce(), Err(DependencyError::Unavailable));
    script.assert_finished();
    drop(key);
    assert_eq!(log.released(), [1, 2]);
}

#[test]
#[should_panic(expected = "script call mismatch")]
fn changing_a_tag_input_is_detected_by_the_script() {
    let log = SecretLog::default();
    let key = log.issue(1);
    let crypto = ScriptedCryptography(Script::new([(
        CryptoCall::VerifyTag {
            key: 1,
            nonce: [0; 24],
            authenticated_data: vec![1, 2],
            tag: [0; 16],
        },
        CryptoReply::Verified(Ok(())),
    )]));
    let _ = crypto.verify_tag(&key, &[0; 24], &[2, 1], &[0; 16]);
}

#[test]
#[should_panic(expected = "expected nonce reply")]
fn mismatched_reply_shapes_are_not_coerced() {
    let mut crypto = ScriptedCryptography(Script::new([(
        CryptoCall::RandomNonce,
        CryptoReply::Verified(Ok(())),
    )]));
    let _ = crypto.random_nonce();
}

#[test]
#[should_panic(expected = "test secret label reused")]
fn secret_labels_cannot_be_reissued_after_drop() {
    let log = SecretLog::default();
    drop(log.issue(1));
    let _ = log.issue(1);
}
