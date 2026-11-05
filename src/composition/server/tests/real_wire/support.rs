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
pub(super) type Crypto = TransportCrypto<ScriptedEntropy>;
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
        let mut crypto = TransportCrypto::new(
            Provider::new(
                ScriptedEntropy(VecDeque::from([
                    Ok(transport(&format!("{local} raw X25519 private-key input"))),
                    Ok(transport(&format!("{local} HELLO nonce"))),
                ])),
                65548,
            )
            .unwrap(),
        );
        let (socket, control) = ScriptedSocket::new();
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
                expected_peer: ExpectedPeer::Unpinned,
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
