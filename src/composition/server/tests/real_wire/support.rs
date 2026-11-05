//! Real primitive/persisted-Identity connection harness with deterministic socket segmentation and independent tags.
pub(super) use crate::modules::cryptography::test_support::transport as vector;
use crate::{
    composition::{cryptography::TransportCrypto, identity::TransportIdentity},
    modules::{
        cryptography::{
            provider::Provider,
            random::SystemEntropy,
            test_support::{ScriptedEntropy, transport},
        },
        identity::{
            api::IdentityProvider,
            authority::{ReadPublic, Sign},
            config::Limits,
            provider::Identity,
            storage::FileStore,
        },
        transport::{
            api::*,
            codec::{FrameHeader, FrameType},
            config::{MAX_FRAME_BYTES, MAX_PAYLOAD_BYTES, ResourceLimits, TransportConfig},
            connection::{Connection, Options},
            handles::StreamOwner,
            stream::registry::Registry,
            test_support::socket::{ReadStep, ScriptedSocket, SocketControl, WriteStep},
        },
    },
};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::AeadInPlace};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Drop for Root {
    // Remove only this harness's owned protected fixture tree after credentials have dropped.
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
pub(super) type RealIdentity = TransportIdentity<FileStore, SystemEntropy>;
type BaseCrypto = TransportCrypto<ScriptedEntropy>;
use crate::modules::transport::ports::{
    DependencyError,
    crypto::{Cryptography, EphemeralKey},
};
use std::sync::{Arc, Mutex};
pub(super) struct Crypto {
    inner: BaseCrypto,
    pub calls: Arc<Mutex<Vec<&'static str>>>,
    control: SocketControl,
}
impl Crypto {
    // Observe real primitive ordering without replacing any verification or derivation result.
    fn new(inner: BaseCrypto, control: SocketControl) -> Self {
        Self {
            inner,
            calls: Arc::new(Mutex::new(Vec::new())),
            control,
        }
    }
}
impl Cryptography for Crypto {
    type EphemeralSecret = <BaseCrypto as Cryptography>::EphemeralSecret;
    type SharedSecret = <BaseCrypto as Cryptography>::SharedSecret;
    type PseudorandomKey = <BaseCrypto as Cryptography>::PseudorandomKey;
    type IntegrityKey = <BaseCrypto as Cryptography>::IntegrityKey;
    // Record this boundary and delegate to the actual native cryptographic provider.
    fn random_nonce(&mut self) -> Result<[u8; 32], DependencyError> {
        self.calls.lock().unwrap().push("nonce");
        self.inner.random_nonce()
    }
    // Record this boundary and delegate to the actual native cryptographic provider.
    fn ephemeral_key(&mut self) -> Result<EphemeralKey<Self::EphemeralSecret>, DependencyError> {
        self.calls.lock().unwrap().push("ephemeral");
        self.inner.ephemeral_key()
    }
    // Record this boundary and delegate to the actual native cryptographic provider.
    fn sha256(&self, bytes: &[u8]) -> Result<[u8; 32], DependencyError> {
        self.calls.lock().unwrap().push("hash");
        self.inner.sha256(bytes)
    }
    // Record this boundary and delegate to the actual native cryptographic provider.
    fn agree(
        &mut self,
        key: Self::EphemeralSecret,
        remote: &[u8; 32],
    ) -> Result<Self::SharedSecret, DependencyError> {
        self.calls.lock().unwrap().push("agree");
        assert!(self.calls.lock().unwrap().contains(&"verified"));
        assert!(self.control.written().len() >= 188);
        self.inner.agree(key, remote)
    }
    // Record this boundary and delegate to the actual native cryptographic provider.
    fn extract(
        &mut self,
        salt: &[u8; 32],
        shared: Self::SharedSecret,
    ) -> Result<Self::PseudorandomKey, DependencyError> {
        self.calls.lock().unwrap().push("extract");
        self.inner.extract(salt, shared)
    }
    // Record this boundary and delegate to the actual native cryptographic provider.
    fn expand_key(
        &mut self,
        key: &Self::PseudorandomKey,
        info: &[u8],
    ) -> Result<Self::IntegrityKey, DependencyError> {
        self.calls.lock().unwrap().push("expand");
        self.inner.expand_key(key, info)
    }
    // Record this boundary and delegate to the actual native cryptographic provider.
    fn expand_nonce_prefix(
        &self,
        key: &Self::PseudorandomKey,
        info: &[u8],
    ) -> Result<[u8; 16], DependencyError> {
        self.calls.lock().unwrap().push("prefix");
        self.inner.expand_nonce_prefix(key, info)
    }
    // Record this boundary and delegate to the actual native cryptographic provider.
    fn tag(
        &self,
        key: &Self::IntegrityKey,
        nonce: &[u8; 24],
        aad: &[u8],
    ) -> Result<[u8; 16], DependencyError> {
        self.calls.lock().unwrap().push("tag");
        self.inner.tag(key, nonce, aad)
    }
    // Record this boundary and delegate to the actual native cryptographic provider.
    fn verify_tag(
        &self,
        key: &Self::IntegrityKey,
        nonce: &[u8; 24],
        aad: &[u8],
        tag: &[u8; 16],
    ) -> Result<(), DependencyError> {
        self.calls.lock().unwrap().push("verify-tag");
        self.inner.verify_tag(key, nonce, aad, tag)
    }

    // Record successful verification separately so agreement cannot precede authenticated peer proof.
    fn verify_signature(
        &self,
        key: &[u8; 32],
        message: &[u8],
        signature: &[u8; 64],
    ) -> Result<(), DependencyError> {
        self.calls.lock().unwrap().push("verify");
        let result = self.inner.verify_signature(key, message, signature);
        if result.is_ok() {
            self.calls.lock().unwrap().push("verified");
        }
        result
    }
}

pub(super) struct Harness {
    pub connection: Connection<Crypto>,
    pub control: SocketControl,
    pub identity: RealIdentity,
    pub crypto: Crypto,
    pub registry: Registry,
    pub owner: StreamOwner,
    pub now: Instant,
    pub receive_sequence: u64,
    pub send_sequence: u64,
    pub role: ConnectionRole,
    _root: Root,
}
// Return the pinned fixture's role spelling, independent of arrival order.
pub(super) fn role_name(role: ConnectionRole) -> &'static str {
    match role {
        ConnectionRole::Initiator => "initiator",
        ConnectionRole::Acceptor => "acceptor",
    }
}
// Encode canonical header and payload without a connection tag.
pub(super) fn aad(kind: FrameType, id: u32, payload: &[u8]) -> Vec<u8> {
    [
        FrameHeader::new(kind, id, payload.len() as u32)
            .unwrap()
            .encode()
            .to_vec(),
        payload.to_vec(),
    ]
    .concat()
}
// Independently construct an empty-plaintext AEAD tag from literal directional key/prefix vectors.
pub(super) fn protected(
    kind: FrameType,
    id: u32,
    payload: &[u8],
    sequence: u64,
    direction: &str,
) -> Vec<u8> {
    let bytes = aad(kind, id, payload);
    let mut nonce = transport(&format!("{direction} nonce prefix"));
    nonce.extend(sequence.to_be_bytes());
    let cipher =
        XChaCha20Poly1305::new_from_slice(&transport(&format!("{direction} key"))).unwrap();
    let tag = cipher
        .encrypt_in_place_detached(XNonce::from_slice(&nonce), &bytes, &mut [])
        .unwrap();
    [bytes, tag.to_vec()].concat()
}
impl Harness {
    // Persist deterministic signing authority, reload it, and compose a real crypto connection before peer input.
    pub fn pending(role: ConnectionRole, write_limit: usize) -> Self {
        Self::configured(role, write_limit, ExpectedPeer::Unpinned)
    }
    // Select explicit expected-ID policy while keeping all primitives and stored credentials real.
    pub fn configured(role: ConnectionRole, write_limit: usize, expected: ExpectedPeer) -> Self {
        let local = role_name(role);
        let path = std::env::temp_dir().join(format!(
            "talksphere-real-wire-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let limits = Limits::new(1024, 64, 65548, 65536, 65536).unwrap();
        let (store, admin) = FileStore::open(&path, limits).unwrap();
        let mut initial = Identity::new(
            store,
            ScriptedEntropy(VecDeque::from([
                Ok(transport(&format!("{local} Ed25519 seed"))),
                Ok(vec![7; 32]),
            ])),
            limits,
            1,
        )
        .unwrap();
        let (handle, _) = initial.create(&admin).unwrap();
        drop(handle);
        drop(initial);
        let (store, admin) = FileStore::open(&path, limits).unwrap();
        let mut provider = Identity::new(store, SystemEntropy, limits, 1).unwrap();
        let (handle, _) = provider.load(&admin).unwrap();
        let read = provider.grant::<ReadPublic>(&admin, &handle, 1).unwrap();
        let sign = provider.grant::<Sign>(&admin, &handle, 65548).unwrap();
        let identity = TransportIdentity::new(provider, handle, read, sign).unwrap();
        let (socket, control) = ScriptedSocket::new();
        let mut crypto = Crypto::new(
            TransportCrypto::new(
                Provider::new(
                    ScriptedEntropy(VecDeque::from([
                        Ok(transport(&format!("{local} raw X25519 private-key input"))),
                        Ok(transport(&format!("{local} HELLO nonce"))),
                    ])),
                    65548,
                )
                .unwrap(),
            ),
            control.clone(),
        );
        for _ in 0..1024 {
            control.push_write(WriteStep::Accept(write_limit));
        }
        let now = Instant::now();
        let config = TransportConfig::new(
            ResourceLimits {
                connections: 2,
                streams_per_connection: 8,
                outbound_bytes: 4 * MAX_FRAME_BYTES,
                buffered_bytes_per_stream: MAX_PAYLOAD_BYTES,
            },
            Duration::from_secs(3),
        )
        .unwrap();
        let connection = Connection::begin(
            Box::new(socket),
            Options {
                role,
                expected_peer: expected,
            },
            config,
            now,
            &identity,
            &mut crypto,
        )
        .unwrap();
        let mut registry = Registry::default();
        let owner = registry
            .register(Protocol {
                id: "cr2se.peer".into(),
                version: 1,
            })
            .unwrap();
        Self {
            connection,
            control,
            identity,
            crypto,
            registry,
            owner,
            now,
            receive_sequence: 0,
            send_sequence: 0,
            role,
            _root: Root(path),
        }
    }
    // Establish a complete real pinned initiator handshake; the registered protocol is test-owned only.
    pub fn new() -> Self {
        let mut h = Self::pending(ConnectionRole::Initiator, usize::MAX);
        let bytes = [
            aad(FrameType::Hello, 0, &transport("acceptor HELLO")),
            aad(FrameType::Auth, 0, &transport("acceptor AUTH payload")),
        ]
        .concat();
        h.control.push_read(ReadStep::Bytes(bytes));
        for _ in 0..32 {
            let _ = h.poll();
            if h.connection.session().is_some() {
                break;
            }
        }
        assert!(h.connection.session().is_some());
        assert!(matches!(
            h.connection.pop_event(),
            Some(Event::Authenticated(_))
        ));
        h
    }
    // Poll bounded connection work without changing the explicitly controlled clock.
    pub fn poll(&mut self) -> Poll<()> {
        self.connection.poll(
            &mut Context::from_waker(Waker::noop()),
            self.now,
            &self.registry,
            &mut self.identity,
            &mut self.crypto,
        )
    }
    // Create valid incoming bytes from the peer's pinned directional material, optionally corrupting the tag.
    pub fn expect_receive(
        &mut self,
        kind: FrameType,
        id: u32,
        payload: &[u8],
        valid: bool,
    ) -> Vec<u8> {
        let direction = if self.role == ConnectionRole::Initiator {
            "acceptor-to-initiator"
        } else {
            "initiator-to-acceptor"
        };
        let mut bytes = protected(kind, id, payload, self.receive_sequence, direction);
        if valid {
            self.receive_sequence += 1;
        } else {
            *bytes.last_mut().unwrap() ^= 1;
        }
        bytes
    }
    // Allow one outgoing frame and compute its exact expected bytes independently of Transport's tag adapter.
    pub fn expect_send(&mut self, kind: FrameType, id: u32, payload: &[u8]) -> Vec<u8> {
        let direction = if self.role == ConnectionRole::Initiator {
            "initiator-to-acceptor"
        } else {
            "acceptor-to-initiator"
        };
        let bytes = protected(kind, id, payload, self.send_sequence, direction);
        self.send_sequence += 1;
        bytes
    }
    // Transfer queued observations without copying private authority or interpreting peer diagnostics.
    pub fn events(&mut self) -> Vec<Event> {
        std::iter::from_fn(|| self.connection.pop_event()).collect()
    }
}

impl Harness {
    // Deliver a correctly tagged state fixture and compare any response against the independent AEAD oracle.
    pub fn receive(
        &mut self,
        kind: FrameType,
        id: u32,
        payload: Vec<u8>,
        response: Option<(FrameType, u32, Vec<u8>)>,
    ) {
        let bytes = self.expect_receive(kind, id, &payload, true);
        let start = self.control.written().len();
        let expected = response
            .map(|(kind, id, payload)| self.expect_send(kind, id, &payload))
            .unwrap_or_default();
        self.control.push_read(ReadStep::Bytes(bytes));
        let _ = self.poll();
        assert_eq!(&self.control.written()[start..], expected);
    }
}
