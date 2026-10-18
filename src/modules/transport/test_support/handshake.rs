//! Strict vector-backed handshake scripts shared by composed connection scenarios.
use crate::modules::transport::{
    api::ConnectionRole,
    codec::{
        FrameHeader, FrameType,
        handshake::{Auth, Hello},
    },
    handshake::Transcript,
    ports::{
        DependencyError,
        crypto::EphemeralKey,
        identity::{IdentityId, PublicIdentity},
    },
    test_support::{
        crypto::{CryptoCall, CryptoReply, ScriptedCryptography, SecretLog},
        identity::{IdentityCall, IdentityReply, ScriptedIdentity},
        script::Script,
        vectors,
    },
};

// Produce exact dependency expectations without simulating cryptographic computation.
pub fn handshake(
    role: ConnectionRole,
) -> (ScriptedIdentity, ScriptedCryptography, SecretLog, Vec<u8>) {
    let same_identity = false;
    let signature_ok = true;
    let agreement_ok = true;
    let (local_name, remote_name) = if role == ConnectionRole::Initiator {
        ("initiator", "acceptor")
    } else {
        ("acceptor", "initiator")
    };
    let local = Hello::decode(&vectors::bytes(&format!("{local_name} HELLO"), 0)).unwrap();
    let mut remote = Hello::decode(&vectors::bytes(&format!("{remote_name} HELLO"), 0)).unwrap();
    let local_id = IdentityId(vectors::array(&format!("{local_name} CR2SE ID")));
    let remote_id = if same_identity {
        remote.identity_public_key = local.identity_public_key;
        local_id
    } else {
        IdentityId(vectors::array(&format!("{remote_name} CR2SE ID")))
    };
    let transcript = match role {
        ConnectionRole::Initiator => Transcript::new(&local, &remote),
        ConnectionRole::Acceptor => Transcript::new(&remote, &local),
    };
    let peer_role = if role == ConnectionRole::Initiator {
        ConnectionRole::Acceptor
    } else {
        ConnectionRole::Initiator
    };
    let local_auth = vectors::array(&format!("{local_name} AUTH payload"));
    let auth = Auth(vectors::array(&format!("{remote_name} AUTH payload")));
    let identity = ScriptedIdentity(Script::new([
        (
            IdentityCall::PublicIdentity,
            IdentityReply::PublicIdentity(Ok(PublicIdentity {
                id: local_id,
                signing_public_key: local.identity_public_key,
            })),
        ),
        (
            IdentityCall::Identify(remote.identity_public_key),
            IdentityReply::IdentityId(Ok(remote_id)),
        ),
        (
            IdentityCall::Sign(transcript.authentication(role).as_bytes().to_vec()),
            IdentityReply::Signature(Ok(local_auth)),
        ),
    ]));
    let secrets = SecretLog::default();
    let salt = vectors::array("HKDF salt");
    let mut calls = vec![
        (
            CryptoCall::EphemeralKey,
            CryptoReply::EphemeralKey(Ok(EphemeralKey {
                public_key: local.ephemeral_public_key,
                private_key: secrets.issue(1),
            })),
        ),
        (
            CryptoCall::RandomNonce,
            CryptoReply::Bytes32(Ok(local.nonce)),
        ),
        (
            CryptoCall::VerifySignature {
                public_key: remote.identity_public_key,
                message: transcript.authentication(peer_role).as_bytes().to_vec(),
                signature: auth.0,
            },
            CryptoReply::Verified(if signature_ok {
                Ok(())
            } else {
                Err(DependencyError::VerificationFailed)
            }),
        ),
    ];
    if signature_ok {
        calls.push((
            CryptoCall::Agree {
                private_key: 1,
                remote_key: remote.ephemeral_public_key,
            },
            CryptoReply::Secret(if agreement_ok {
                Ok(secrets.issue(2))
            } else {
                Err(DependencyError::VerificationFailed)
            }),
        ));
        if agreement_ok {
            calls.extend([
                (
                    CryptoCall::Sha256(
                        [b"CR2SE-NETWORK-KEYS-V1\0".as_slice(), transcript.as_bytes()].concat(),
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
                    CryptoReply::Bytes16(Ok([6; 16])),
                ),
                (
                    CryptoCall::ExpandNoncePrefix {
                        key: 3,
                        info: b"CR2SE-NETWORK-V1-A2I-NONCE".to_vec(),
                    },
                    CryptoReply::Bytes16(Ok([7; 16])),
                ),
            ]);
        }
    }

    let incoming = [
        FrameHeader::new(FrameType::Hello, 0, 100)
            .unwrap()
            .encode()
            .to_vec(),
        remote.encode().to_vec(),
        FrameHeader::new(FrameType::Auth, 0, 64)
            .unwrap()
            .encode()
            .to_vec(),
        auth.encode().to_vec(),
    ]
    .concat();
    (
        identity,
        ScriptedCryptography(Script::new(calls)),
        secrets,
        incoming,
    )
}
