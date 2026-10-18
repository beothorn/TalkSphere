//! TR-04/TR-05 ordering and identity gates using strict primitive scripts.
use super::*;
use crate::modules::transport::{
    framing::FrameReader,
    ports::{crypto::EphemeralKey, identity::PublicIdentity},
    test_support::{
        crypto::{CryptoCall, CryptoReply, ScriptedCryptography, SecretLog},
        identity::{IdentityCall, IdentityReply, ScriptedIdentity},
        script::Script,
        socket::{ReadStep, ScriptedSocket},
        vectors,
    },
};
use std::task::{Context, Poll, Waker};

struct Fixture {
    state: Handshake<ScriptedCryptography>,
    hello: WireFrame,
    remote: Hello,
    auth: Auth,
    identity: ScriptedIdentity,
    crypto: ScriptedCryptography,
    secrets: SecretLog,
    remote_id: IdentityId,
}

// Credential and entropy failures cannot create a handshake or retain an ephemeral secret.
#[test]
fn startup_failures_release_fresh_material() {
    let identity = ScriptedIdentity(Script::new([(
        IdentityCall::PublicIdentity,
        IdentityReply::PublicIdentity(Err(DependencyError::Unavailable)),
    )]));
    let mut crypto = ScriptedCryptography(Script::new([]));
    assert_eq!(
        Handshake::begin(
            ConnectionRole::Initiator,
            ExpectedPeer::Unpinned,
            &identity,
            &mut crypto
        )
        .err(),
        Some(HandshakeError::Dependency(DependencyError::Unavailable))
    );
    let secrets = SecretLog::default();
    let identity = ScriptedIdentity(Script::new([(
        IdentityCall::PublicIdentity,
        IdentityReply::PublicIdentity(Ok(PublicIdentity {
            id: IdentityId([1; 32]),
            signing_public_key: [2; 32],
        })),
    )]));
    let mut crypto = ScriptedCryptography(Script::new([
        (
            CryptoCall::EphemeralKey,
            CryptoReply::EphemeralKey(Ok(EphemeralKey {
                public_key: [3; 32],
                private_key: secrets.issue(1),
            })),
        ),
        (
            CryptoCall::RandomNonce,
            CryptoReply::Bytes32(Err(DependencyError::Unavailable)),
        ),
    ]));
    assert_eq!(
        Handshake::begin(
            ConnectionRole::Initiator,
            ExpectedPeer::Unpinned,
            &identity,
            &mut crypto
        )
        .err(),
        Some(HandshakeError::Dependency(DependencyError::Unavailable))
    );
    assert_eq!(secrets.released(), [1]);
    crypto.0.assert_finished();
}

// A correct pin succeeds, but repeated AUTH after verification invalidates the connection.
#[test]
fn pinned_peer_succeeds_and_duplicate_auth_releases_session() {
    let expected = ExpectedPeer::Known(IdentityId(vectors::array("acceptor CR2SE ID")));
    let mut f = fixture(ConnectionRole::Initiator, false, true, true, expected);
    f.state.sent(FrameType::Hello).unwrap();
    f.state
        .receive(
            raw_frame(FrameType::Hello, f.remote.encode().to_vec()).unwrap(),
            &mut f.identity,
            &mut f.crypto,
        )
        .unwrap();
    f.state.sent(FrameType::Auth).unwrap();
    f.state
        .receive(
            raw_frame(FrameType::Auth, f.auth.encode().to_vec()).unwrap(),
            &mut f.identity,
            &mut f.crypto,
        )
        .unwrap();
    assert!(f.state.is_authenticated());
    assert_eq!(
        f.state
            .receive(
                raw_frame(FrameType::Auth, f.auth.encode().to_vec()).unwrap(),
                &mut f.identity,
                &mut f.crypto
            )
            .err(),
        Some(HandshakeError::Order)
    );
    assert!(!f.state.is_authenticated());
    assert!(f.state.take_established().is_none());
    assert_eq!(f.secrets.released().len(), 5);
}

// Build exact role-aware primitive expectations; only outcomes, not real signatures, are simulated.
fn fixture(
    role: ConnectionRole,
    same_identity: bool,
    signature_ok: bool,
    agreement_ok: bool,
    expected: ExpectedPeer,
) -> Fixture {
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
    let mut crypto = ScriptedCryptography(Script::new(calls));
    let (state, hello) = Handshake::begin(role, expected, &identity, &mut crypto).unwrap();
    assert_eq!(hello.payload(), local.encode());
    Fixture {
        state,
        hello,
        remote,
        auth,
        identity,
        crypto,
        secrets,
        remote_id,
    }
}

// Both roles may receive HELLO early; AUTH send and peer verification independently gate tag phases.
#[test]
fn both_roles_and_same_identity_peers_authenticate_only_after_both_checks() {
    for role in [ConnectionRole::Initiator, ConnectionRole::Acceptor] {
        for same in [false, true] {
            let mut f = fixture(role, same, true, true, ExpectedPeer::Unpinned);
            assert!(f.state.take_established().is_none());
            assert_eq!(f.hello.header().frame_type(), FrameType::Hello);
            let auth = f
                .state
                .receive(
                    raw_frame(FrameType::Hello, f.remote.encode().to_vec()).unwrap(),
                    &mut f.identity,
                    &mut f.crypto,
                )
                .unwrap()
                .unwrap();
            assert_eq!(auth.header().frame_type(), FrameType::Auth);
            assert_eq!(f.state.send_protection(), FrameProtection::Untagged);
            assert_eq!(f.state.receive_protection(), FrameProtection::Untagged);
            f.state.sent(FrameType::Hello).unwrap();
            // In one order peer proof arrives before local AUTH write; in the other, after.
            if same {
                f.state.sent(FrameType::Auth).unwrap();
            }
            assert!(!f.state.is_authenticated());
            f.state
                .receive(
                    raw_frame(FrameType::Auth, f.auth.encode().to_vec()).unwrap(),
                    &mut f.identity,
                    &mut f.crypto,
                )
                .unwrap();
            assert_eq!(f.state.receive_protection(), FrameProtection::Tagged);
            if !same {
                assert!(!f.state.is_authenticated());
                f.state.sent(FrameType::Auth).unwrap();
            }
            assert!(f.state.is_authenticated());
            assert_eq!(f.state.send_protection(), FrameProtection::Tagged);
            let established = f.state.take_established().unwrap();
            assert_eq!(established.peer_identity, f.remote_id);
            assert_eq!(established.role, role);
            if same {
                assert_eq!(established.local_identity, established.peer_identity);
            }
            assert!(f.state.take_established().is_none());
            f.identity.0.assert_finished();
            f.crypto.0.assert_finished();
        }
    }
}
// Expected contacts are checked before signing; bad signatures/agreements release ephemeral state.
#[test]
fn expected_identity_signature_and_agreement_failures_never_emit_context() {
    let mut f = fixture(
        ConnectionRole::Initiator,
        false,
        true,
        true,
        ExpectedPeer::Known(IdentityId([99; 32])),
    );
    assert_eq!(
        f.state
            .receive(
                raw_frame(FrameType::Hello, f.remote.encode().to_vec()).unwrap(),
                &mut f.identity,
                &mut f.crypto
            )
            .err(),
        Some(HandshakeError::UnexpectedIdentity)
    );
    assert!(f.state.take_established().is_none());
    assert!(f.secrets.released().contains(&1));
    for (signature_ok, agreement_ok) in [(false, true), (true, false)] {
        let mut f = fixture(
            ConnectionRole::Initiator,
            false,
            signature_ok,
            agreement_ok,
            ExpectedPeer::Unpinned,
        );
        f.state
            .receive(
                raw_frame(FrameType::Hello, f.remote.encode().to_vec()).unwrap(),
                &mut f.identity,
                &mut f.crypto,
            )
            .unwrap();
        assert_eq!(
            f.state
                .receive(
                    raw_frame(FrameType::Auth, f.auth.encode().to_vec()).unwrap(),
                    &mut f.identity,
                    &mut f.crypto
                )
                .err(),
            Some(HandshakeError::Dependency(
                DependencyError::VerificationFailed
            ))
        );
        assert!(f.state.take_established().is_none());
        assert!(f.secrets.released().contains(&1));
        f.crypto.0.assert_finished();
    }
}
// Duplicate/out-of-order/application frames and incorrect local completion reports fail permanently.
#[test]
fn ordering_duplicates_and_payload_failures_are_terminal() {
    for kind in [FrameType::Auth, FrameType::Ping] {
        let mut f = fixture(
            ConnectionRole::Initiator,
            false,
            true,
            true,
            ExpectedPeer::Unpinned,
        );
        assert_eq!(
            f.state
                .receive(
                    raw_frame(kind, vec![0; 8]).unwrap(),
                    &mut f.identity,
                    &mut f.crypto
                )
                .err(),
            Some(HandshakeError::Order)
        );
        assert_eq!(f.state.sent(FrameType::Hello), Err(HandshakeError::Order));
    }
    let mut f = fixture(
        ConnectionRole::Initiator,
        false,
        true,
        true,
        ExpectedPeer::Unpinned,
    );
    assert_eq!(f.state.sent(FrameType::Auth), Err(HandshakeError::Order));
    let mut f = fixture(
        ConnectionRole::Initiator,
        false,
        true,
        true,
        ExpectedPeer::Unpinned,
    );
    assert!(matches!(
        f.state.receive(
            raw_frame(FrameType::Hello, vec![0; 99]).unwrap(),
            &mut f.identity,
            &mut f.crypto
        ),
        Err(HandshakeError::Payload(_))
    ));
    let mut f = fixture(
        ConnectionRole::Initiator,
        false,
        true,
        true,
        ExpectedPeer::Unpinned,
    );
    f.state
        .receive(
            raw_frame(FrameType::Hello, f.remote.encode().to_vec()).unwrap(),
            &mut f.identity,
            &mut f.crypto,
        )
        .unwrap();
    assert_eq!(
        f.state
            .receive(
                raw_frame(FrameType::Hello, f.remote.encode().to_vec()).unwrap(),
                &mut f.identity,
                &mut f.crypto
            )
            .err(),
        Some(HandshakeError::Order)
    );
}
// A single TCP chunk containing AUTH and a tagged frame is split at the phase boundary.
#[test]
fn auth_then_tagged_frame_in_one_socket_chunk_uses_separate_receive_phases() {
    let mut f = fixture(
        ConnectionRole::Initiator,
        false,
        true,
        true,
        ExpectedPeer::Unpinned,
    );
    f.state.sent(FrameType::Hello).unwrap();
    f.state
        .receive(
            raw_frame(FrameType::Hello, f.remote.encode().to_vec()).unwrap(),
            &mut f.identity,
            &mut f.crypto,
        )
        .unwrap();
    f.state.sent(FrameType::Auth).unwrap();
    let bytes = [
        FrameHeader::new(FrameType::Auth, 0, 64)
            .unwrap()
            .encode()
            .as_slice(),
        f.auth.0.as_slice(),
        vectors::bytes("complete bytes written to TCP", 1).as_slice(),
    ]
    .concat();
    let (mut socket, control) = ScriptedSocket::new();
    control.push_read(ReadStep::Bytes(bytes));
    let mut reader = FrameReader::default();
    let mut cx = Context::from_waker(Waker::noop());
    let Poll::Ready(Ok(Some(auth))) =
        reader.poll_frame(&mut socket, &mut cx, f.state.receive_protection())
    else {
        panic!("AUTH not read");
    };
    f.state
        .receive(auth, &mut f.identity, &mut f.crypto)
        .unwrap();
    let Poll::Ready(Ok(Some(next))) =
        reader.poll_frame(&mut socket, &mut cx, f.state.receive_protection())
    else {
        panic!("tagged frame not read");
    };
    assert_eq!(next.header().frame_type(), FrameType::Pong);
    assert!(next.tag().is_some());
    control.assert_finished();
}
