//! Actual Transport handshakes and protected frames with test-owned real signing credentials.
use super::*;
use crate::modules::{
    cryptography::{
        credential,
        primitives::signature,
        secrets::Signing,
        test_support::{ScriptedEntropy, hex, observe_releases, transport},
    },
    transport::{
        api::{ConnectionRole, ExpectedPeer},
        codec::{FrameHeader, FrameType, handshake::Hello},
        config::MAX_PAYLOAD_BYTES,
        framing::WireFrame,
        handshake::{Handshake, HandshakeError},
        integrity::Integrity as IntegrityState,
        ports::identity::{AuthenticationMessage, Identity, IdentityId, PublicIdentity},
    },
};
use std::{collections::VecDeque, sync::atomic::Ordering};
use zeroize::Zeroizing;

struct TestIdentity {
    public: [u8; 32],
    private: Secret<Signing>,
}
impl Identity for TestIdentity {
    // Existing test credentials are exposed without persistence or invented identity authority.
    fn public_identity(&self) -> Result<PublicIdentity, DependencyError> {
        Ok(PublicIdentity {
            id: self.identify(&self.public)?,
            signing_public_key: self.public,
        })
    }
    // Compute the shared public formula; proof remains the handshake's responsibility.
    fn identify(&self, key: &[u8; 32]) -> Result<IdentityId, DependencyError> {
        credential::identity_id(key)
            .map(IdentityId)
            .map_err(map_error)
    }
    // Only a Transport-created scoped message can request a real test signature.
    fn sign_authentication(
        &mut self,
        message: &AuthenticationMessage,
    ) -> Result<[u8; 64], DependencyError> {
        signature::sign(&self.private, message.as_bytes(), 1024).map_err(map_error)
    }
}
// Import pinned private values only into a test-owned Identity port.
fn identity(role: &str) -> TestIdentity {
    TestIdentity {
        public: transport(&format!("{role} Ed25519 public key"))
            .try_into()
            .unwrap(),
        private: Secret::new(Zeroizing::new(
            transport(&format!("{role} Ed25519 seed"))
                .try_into()
                .unwrap(),
        )),
    }
}
// Use fixed entropy to compare actual signatures, derived material and tags with literal vectors.
fn crypto(role: &str) -> TransportCrypto<ScriptedEntropy> {
    let entropy = ScriptedEntropy(VecDeque::from([
        Ok(transport(&format!("{role} raw X25519 private-key input"))),
        Ok(transport(&format!("{role} HELLO nonce"))),
    ]));
    TransportCrypto::new(Provider::new(entropy, MAX_PAYLOAD_BYTES + 12).unwrap())
}
// Construct a structurally correct untagged frame for the actual state machine.
fn raw(kind: FrameType, payload: Vec<u8>) -> WireFrame {
    WireFrame::new(
        FrameHeader::new(kind, 0, payload.len() as u32).unwrap(),
        payload,
        None,
    )
    .unwrap()
}
// Preserve exact observed bytes for an independent later replay attempt.
fn copy_frame(frame: &WireFrame) -> WireFrame {
    WireFrame::new(
        frame.header(),
        frame.payload().to_vec(),
        frame.tag().copied(),
    )
    .unwrap()
}

// Real primitives complete both roles and reproduce both directional first-frame vectors.
#[test]
fn real_handshake_and_bidirectional_frames_match_pinned_vectors() {
    let mut i = identity("initiator");
    let mut a = identity("acceptor");
    let mut ci = crypto("initiator");
    let mut ca = crypto("acceptor");
    let (mut hi, hello_i) = Handshake::begin(
        ConnectionRole::Initiator,
        ExpectedPeer::Known(a.public_identity().unwrap().id),
        &i,
        &mut ci,
    )
    .unwrap();
    let (mut ha, hello_a) = Handshake::begin(
        ConnectionRole::Acceptor,
        ExpectedPeer::Known(i.public_identity().unwrap().id),
        &a,
        &mut ca,
    )
    .unwrap();
    assert_eq!(hello_i.payload(), transport("initiator HELLO"));
    assert_eq!(hello_a.payload(), transport("acceptor HELLO"));
    hi.sent(FrameType::Hello).unwrap();
    ha.sent(FrameType::Hello).unwrap();
    let auth_i = hi.receive(hello_a, &mut i, &mut ci).unwrap().unwrap();
    let auth_a = ha.receive(hello_i, &mut a, &mut ca).unwrap().unwrap();
    assert_eq!(auth_i.payload(), transport("initiator AUTH payload"));
    assert_eq!(auth_a.payload(), transport("acceptor AUTH payload"));
    hi.sent(FrameType::Auth).unwrap();
    ha.sent(FrameType::Auth).unwrap();
    assert!(!hi.is_authenticated());
    hi.receive(auth_a, &mut i, &mut ci).unwrap();
    ha.receive(auth_i, &mut a, &mut ca).unwrap();
    let established_i = hi.take_established().unwrap();
    let established_a = ha.take_established().unwrap();
    assert_eq!(
        established_i.keys.send.nonce_prefix.as_slice(),
        transport("initiator-to-acceptor nonce prefix")
    );
    assert_eq!(
        established_a.keys.send.nonce_prefix.as_slice(),
        transport("acceptor-to-initiator nonce prefix")
    );
    let mut si = IntegrityState::new(established_i.keys);
    let mut sa = IntegrityState::new(established_a.keys);
    let ping = si
        .seal(raw(FrameType::Ping, transport("payload")), &ci)
        .unwrap();
    assert_eq!(ping.tag().unwrap().as_slice(), transport("integrity tag"));
    let replay = copy_frame(&ping);
    si.sent().unwrap();
    assert_eq!(
        sa.verify(ping, &ca).unwrap().payload(),
        transport("payload")
    );
    let pong = sa
        .seal(raw(FrameType::Pong, transport("payload")), &ca)
        .unwrap();
    assert_eq!(
        pong.tag().unwrap().as_slice(),
        hex("5feb9216750a6618af80f39b7f813ea6")
    );
    sa.sent().unwrap();
    si.verify(pong, &ci).unwrap();
    let data = WireFrame::new(
        FrameHeader::new(FrameType::Data, 1, MAX_PAYLOAD_BYTES as u32).unwrap(),
        vec![5; MAX_PAYLOAD_BYTES],
        None,
    )
    .unwrap();
    let data = si.seal(data, &ci).unwrap();
    si.sent().unwrap();
    assert_eq!(
        sa.verify(data, &ca).unwrap().payload().len(),
        MAX_PAYLOAD_BYTES
    );
    assert!(sa.verify(replay, &ca).is_err());
    assert!(sa.is_closed());
}

// A bad real AUTH releases only the initial ephemeral key: no shared/PRK/session secrets were created.
#[test]
fn real_auth_failure_never_derives_session_material() {
    let mut identity = identity("initiator");
    let mut crypto = crypto("initiator");
    let (result, releases) = observe_releases(|| {
        let (mut handshake, _) = Handshake::begin(
            ConnectionRole::Initiator,
            ExpectedPeer::Unpinned,
            &identity,
            &mut crypto,
        )
        .unwrap();
        handshake.sent(FrameType::Hello).unwrap();
        handshake
            .receive(
                raw(FrameType::Hello, transport("acceptor HELLO")),
                &mut identity,
                &mut crypto,
            )
            .unwrap();
        handshake.sent(FrameType::Auth).unwrap();
        let mut auth = transport("acceptor AUTH payload");
        auth[0] ^= 1;
        let result = handshake.receive(raw(FrameType::Auth, auth), &mut identity, &mut crypto);
        assert!(handshake.take_established().is_none());
        result.err()
    });
    assert_eq!(
        result,
        Some(HandshakeError::Dependency(
            DependencyError::VerificationFailed
        ))
    );
    assert_eq!(releases.load(Ordering::SeqCst), 1);
}

// Error mappings preserve the dependency contract, and actual adapter failures have no fallback.
#[test]
fn adapter_failures_and_port_limits() {
    for (input, expected) in [
        (CryptoError::InvalidInput, DependencyError::InvalidInput),
        (CryptoError::ResourceLimit, DependencyError::InvalidInput),
        (
            CryptoError::AuthenticationFailed,
            DependencyError::VerificationFailed,
        ),
        (CryptoError::AccessDenied, DependencyError::Denied),
        (CryptoError::Unavailable, DependencyError::Unavailable),
    ] {
        assert_eq!(map_error(input), expected);
    }
    let mut adapter =
        TransportCrypto::new(Provider::new(ScriptedEntropy(VecDeque::new()), 148).unwrap());
    assert_eq!(adapter.random_nonce(), Err(DependencyError::Unavailable));
    assert!(adapter.ephemeral_key().is_err());
    assert_eq!(
        adapter.sha256(&[0; 149]),
        Err(DependencyError::InvalidInput)
    );
    let key = Secret::new(Zeroizing::new([1; 32]));
    assert!(adapter.agree(key, &[0; 32]).is_err());
    let hello = Hello::decode(&transport("acceptor HELLO")).unwrap();
    assert_eq!(hello.nonce.as_slice(), transport("acceptor HELLO nonce"));
}
