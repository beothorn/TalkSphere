//! Canonical construction tests; scripted outputs do not prove real cryptography.
use super::*;
use crate::modules::transport::{
    api::ConnectionRole,
    codec::handshake::{Auth, Hello},
    ports::DependencyError,
    test_support::{
        crypto::{CryptoCall, CryptoReply, ScriptedCryptography, SecretLog},
        identity::{IdentityCall, IdentityReply, ScriptedIdentity},
        script::Script,
        vectors,
    },
};

// Use published payloads, independent of the Transcript encoder under test.
fn transcript_fixture() -> (Transcript, Vec<u8>) {
    let i = vectors::bytes("initiator HELLO", 0);
    let a = vectors::bytes("acceptor HELLO", 0);
    let expected = [&[0, 0, 0, 100], i.as_slice(), &[0, 0, 0, 100], a.as_slice()].concat();
    (
        Transcript::new(&Hello::decode(&i).unwrap(), &Hello::decode(&a).unwrap()),
        expected,
    )
}
// Both roles sign/verify distinct domain-separated messages with exact published payload ordering.
#[test]
fn canonical_transcript_and_role_bound_signatures_are_requested() {
    let (transcript, expected) = transcript_fixture();
    assert_eq!(transcript.as_bytes().as_slice(), expected);
    for (role, byte, name) in [
        (ConnectionRole::Initiator, 1, "initiator"),
        (ConnectionRole::Acceptor, 2, "acceptor"),
    ] {
        let message = [
            b"CR2SE-NETWORK-AUTH-V1\0".as_slice(),
            &[byte],
            expected.as_slice(),
        ]
        .concat();
        assert_eq!(transcript.authentication(role).as_bytes(), message);
        let signature = vectors::array(&format!("{name} AUTH payload"));
        let public_key = vectors::array(&format!("{name} Ed25519 public key"));
        let signing = Script::new([(
            IdentityCall::Sign(message.clone()),
            IdentityReply::Signature(Ok(signature)),
        )]);
        assert_eq!(
            transcript
                .sign(role, &mut ScriptedIdentity(signing.clone()))
                .unwrap()
                .0,
            signature
        );
        signing.assert_finished();
        let verify = Script::new([(
            CryptoCall::VerifySignature {
                public_key,
                message,
                signature,
            },
            CryptoReply::Verified(Ok(())),
        )]);
        transcript
            .verify(
                role,
                &public_key,
                &Auth(signature),
                &ScriptedCryptography(verify.clone()),
            )
            .unwrap();
        verify.assert_finished();
    }
}
// All HKDF labels/inputs and direction assignments are checked against independently assembled calls.
#[test]
fn keys_follow_exact_primitive_requests_and_opposite_role_assignments() {
    for role in [ConnectionRole::Initiator, ConnectionRole::Acceptor] {
        let (transcript, expected) = transcript_fixture();
        let secrets = SecretLog::default();
        let remote = vectors::array("acceptor ephemeral X25519 public key");
        let salt = vectors::array("HKDF salt");
        let i2a = vectors::array("initiator-to-acceptor nonce prefix");
        let a2i = vectors::array("acceptor-to-initiator nonce prefix");
        let script = Script::new([
            (
                CryptoCall::Agree {
                    private_key: 1,
                    remote_key: remote,
                },
                CryptoReply::Secret(Ok(secrets.issue(2))),
            ),
            (
                CryptoCall::Sha256(
                    [b"CR2SE-NETWORK-KEYS-V1\0".as_slice(), expected.as_slice()].concat(),
                ),
                CryptoReply::Bytes32(Ok(salt)),
            ),
            (
                CryptoCall::Extract { salt, shared: 2 },
                CryptoReply::Secret(Ok(secrets.issue(3))),
            ),
            (
                CryptoCall::ExpandKey {
                    key: 3,
                    info: b"CR2SE-NETWORK-V1-I2A-KEY".to_vec(),
                },
                CryptoReply::Secret(Ok(secrets.issue(4))),
            ),
            (
                CryptoCall::ExpandKey {
                    key: 3,
                    info: b"CR2SE-NETWORK-V1-A2I-KEY".to_vec(),
                },
                CryptoReply::Secret(Ok(secrets.issue(5))),
            ),
            (
                CryptoCall::ExpandNoncePrefix {
                    key: 3,
                    info: b"CR2SE-NETWORK-V1-I2A-NONCE".to_vec(),
                },
                CryptoReply::Bytes16(Ok(i2a)),
            ),
            (
                CryptoCall::ExpandNoncePrefix {
                    key: 3,
                    info: b"CR2SE-NETWORK-V1-A2I-NONCE".to_vec(),
                },
                CryptoReply::Bytes16(Ok(a2i)),
            ),
        ]);
        let keys = derive_keys(
            role,
            secrets.issue(1),
            &remote,
            &transcript,
            &mut ScriptedCryptography(script.clone()),
        )
        .unwrap();
        let (send, receive, prefix) = if role == ConnectionRole::Initiator {
            (4, 5, i2a)
        } else {
            (5, 4, a2i)
        };
        assert_eq!(keys.send.key.label(), send);
        assert_eq!(keys.receive.key.label(), receive);
        assert_eq!(keys.send.nonce_prefix, prefix);
        assert_eq!(
            keys.receive.nonce_prefix,
            if role == ConnectionRole::Initiator {
                a2i
            } else {
                i2a
            }
        );
        assert_eq!(secrets.released(), [1, 2, 3]);
        drop(keys);
        assert_eq!(secrets.released().len(), 5);
        script.assert_finished();
    }
    let prefix = vectors::array("initiator-to-acceptor nonce prefix");
    assert_eq!(
        nonce(&prefix, 0).as_slice(),
        vectors::bytes("nonce prefix || uint64_be(0)", 0)
    );
    assert_eq!(
        &nonce(&prefix, 0x0102_0304_0506_0708)[16..],
        &[1, 2, 3, 4, 5, 6, 7, 8]
    );
    assert_eq!(&nonce(&prefix, u64::MAX)[16..], &[255; 8]);
}
// Refused signatures and all-zero agreement rejection propagate without synthetic success.
#[test]
fn dependency_failures_preserve_their_causes_and_release_ephemeral_material() {
    let (transcript, _) = transcript_fixture();
    let role = ConnectionRole::Initiator;
    let message = transcript.authentication(role).as_bytes().to_vec();
    let signing = Script::new([(
        IdentityCall::Sign(message.clone()),
        IdentityReply::Signature(Err(DependencyError::Denied)),
    )]);
    assert_eq!(
        transcript.sign(role, &mut ScriptedIdentity(signing)).err(),
        Some(DependencyError::Denied)
    );
    let verify = Script::new([(
        CryptoCall::VerifySignature {
            public_key: [0; 32],
            message,
            signature: [0; 64],
        },
        CryptoReply::Verified(Err(DependencyError::VerificationFailed)),
    )]);
    assert_eq!(
        transcript.verify(
            role,
            &[0; 32],
            &Auth([0; 64]),
            &ScriptedCryptography(verify)
        ),
        Err(DependencyError::VerificationFailed)
    );
    let secrets = SecretLog::default();
    let agree = Script::new([(
        CryptoCall::Agree {
            private_key: 1,
            remote_key: [0; 32],
        },
        CryptoReply::Secret(Err(DependencyError::VerificationFailed)),
    )]);
    assert_eq!(
        derive_keys(
            role,
            secrets.issue(1),
            &[0; 32],
            &transcript,
            &mut ScriptedCryptography(agree)
        )
        .err(),
        Some(DependencyError::VerificationFailed)
    );
    assert_eq!(secrets.released(), [1]);
}
