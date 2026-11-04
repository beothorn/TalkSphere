//! Actual Transport handshakes and protected frames with test-owned real signing credentials.
use super::*;
use crate::composition::cryptography::TransportCrypto;
use crate::modules::cryptography::provider::Provider;
use crate::modules::{
    cryptography::test_support::{ScriptedEntropy, hex, transport},
    transport::{
        api::{ConnectionRole, ExpectedPeer},
        codec::{FrameHeader, FrameType},
        config::MAX_PAYLOAD_BYTES,
        framing::WireFrame,
        handshake::Handshake,
        integrity::Integrity as IntegrityState,
        ports::identity::{AuthenticationMessage, Identity, IdentityId, PublicIdentity},
    },
};
use std::collections::VecDeque;

use crate::modules::identity::{
    api::IdentityProvider,
    authority::{ReadPublic, Sign},
    config::Limits,
    storage::FileStore,
};
// Keep each protected test store alive and remove its private fixture files when the adapter drops.
struct TestIdentity {
    inner: TransportIdentity<FileStore, ScriptedEntropy>,
    root: std::path::PathBuf,
}
impl Drop for TestIdentity {
    // Remove only this test's uniquely owned directory after its last operation.
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
impl Identity for TestIdentity {
    // Delegate public access to the real scoped Identity adapter.
    fn public_identity(&self) -> Result<PublicIdentity, DependencyError> {
        self.inner.public_identity()
    }
    // Delegate public ID derivation without creating authentication state.
    fn identify(&self, key: &[u8; 32]) -> Result<IdentityId, DependencyError> {
        self.inner.identify(key)
    }
    // Delegate only Transport-constructed authentication messages.
    fn sign_authentication(
        &mut self,
        message: &AuthenticationMessage,
    ) -> Result<[u8; 64], DependencyError> {
        self.inner.sign_authentication(message)
    }
}
// Persist pinned test keys, close the provider and reload them before composing the actual Transport port.
fn identity(role: &str) -> TestIdentity {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "talksphere-adapter-{}-{role}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    let limits = Limits::new(256, 8, 4096, 8192, 4096).unwrap();
    let (store, admin) = FileStore::open(&root, limits).unwrap();
    let entropy = ScriptedEntropy(VecDeque::from([
        Ok(transport(&format!("{role} Ed25519 seed"))),
        Ok(vec![7; 32]),
    ]));
    let mut provider =
        crate::modules::identity::provider::Identity::new(store, entropy, limits, 1).unwrap();
    let (handle, _) = provider.create(&admin).unwrap();
    drop(handle);
    drop(provider);
    let (store, admin) = FileStore::open(&root, limits).unwrap();
    let mut provider = crate::modules::identity::provider::Identity::new(
        store,
        ScriptedEntropy(VecDeque::new()),
        limits,
        1,
    )
    .unwrap();
    let (handle, _) = provider.load(&admin).unwrap();
    let read = provider.grant::<ReadPublic>(&admin, &handle, 4096).unwrap();
    let sign = provider.grant::<Sign>(&admin, &handle, 4096).unwrap();
    TestIdentity {
        inner: TransportIdentity::new(provider, handle, read, sign).unwrap(),
        root,
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

// Denied signing and an unexpected contact ID cannot publish authenticated session keys.
#[test]
fn denied_authority_and_contact_mismatch_stop_authentication() {
    let mut i = identity("initiator");
    let a = identity("acceptor");
    let mut ci = crypto("initiator");
    let mut ca = crypto("acceptor");
    let (mut hi, _) = Handshake::begin(
        ConnectionRole::Initiator,
        ExpectedPeer::Known(a.public_identity().unwrap().id),
        &i,
        &mut ci,
    )
    .unwrap();
    let (_, hello) = Handshake::begin(
        ConnectionRole::Acceptor,
        ExpectedPeer::Known(i.public_identity().unwrap().id),
        &a,
        &mut ca,
    )
    .unwrap();
    hi.sent(FrameType::Hello).unwrap();
    let foreign = crate::modules::identity::authority::Authority::new();
    i.inner.sign = foreign
        .grant(
            i.inner
                .provider
                .public(&i.inner.handle, &i.inner.read)
                .unwrap()
                .identity
                .id(),
            4096,
        )
        .unwrap();
    assert!(hi.receive(hello, &mut i, &mut ci).is_err());
    assert!(!hi.is_authenticated());
    assert!(hi.take_established().is_none());
    let mut i = identity("initiator");
    let mut ci = crypto("initiator");
    let mut ca = crypto("acceptor");
    let (mut hi, _) = Handshake::begin(
        ConnectionRole::Initiator,
        ExpectedPeer::Known(IdentityId([0; 32])),
        &i,
        &mut ci,
    )
    .unwrap();
    let (_, hello) = Handshake::begin(
        ConnectionRole::Acceptor,
        ExpectedPeer::Known(i.public_identity().unwrap().id),
        &a,
        &mut ca,
    )
    .unwrap();
    hi.sent(FrameType::Hello).unwrap();
    assert!(hi.receive(hello, &mut i, &mut ci).is_err());
    assert!(!hi.is_authenticated());
    assert!(hi.take_established().is_none());
}
