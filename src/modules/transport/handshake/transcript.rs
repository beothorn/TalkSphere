//! Canonical role-ordered HELLO transcript and purpose-scoped AUTH messages.
use crate::modules::transport::{
    api::ConnectionRole,
    codec::handshake::{Auth, Hello},
    ports::{
        DependencyError,
        crypto::Cryptography,
        identity::{AuthenticationMessage, Identity},
    },
};

/// The two complete HELLO payloads, initiator first, each prefixed by uint32 length.
pub struct Transcript([u8; 208]);

impl Transcript {
    /// Order inputs by TCP role, never by arrival time.
    pub fn new(initiator: &Hello, acceptor: &Hello) -> Self {
        tracing::trace!(
            function = "Transcript::new",
            "Ordering HELLO payloads by role"
        );
        let mut bytes = [0; 208];
        bytes[..4].copy_from_slice(&100u32.to_be_bytes());
        bytes[4..104].copy_from_slice(&initiator.encode());
        bytes[104..108].copy_from_slice(&100u32.to_be_bytes());
        bytes[108..].copy_from_slice(&acceptor.encode());
        Self(bytes)
    }
    /// Borrow canonical bytes for the signature and key derivation constructions.
    pub fn as_bytes(&self) -> &[u8; 208] {
        tracing::trace!(
            function = "Transcript::as_bytes",
            "Borrowing canonical transcript bytes"
        );
        &self.0
    }
    /// Construct the exact domain-separated signing capability for one sender role.
    pub fn authentication(&self, role: ConnectionRole) -> AuthenticationMessage {
        tracing::trace!(
            function = "Transcript::authentication",
            "Constructing role-bound authentication bytes"
        );
        let role = match role {
            ConnectionRole::Initiator => 1,
            ConnectionRole::Acceptor => 2,
        };
        let bytes = [
            b"CR2SE-NETWORK-AUTH-V1".as_slice(),
            &[0, role],
            self.0.as_slice(),
        ]
        .concat();
        AuthenticationMessage { bytes }
    }
    /// Ask Identity to sign only this canonical role-bound message.
    /// # Errors
    /// Propagates signing refusal or dependency failure without a fallback signature.
    pub fn sign(
        &self,
        role: ConnectionRole,
        identity: &mut impl Identity,
    ) -> Result<Auth, DependencyError> {
        tracing::trace!(
            function = "Transcript::sign",
            "Requesting a Transport authentication signature"
        );
        identity
            .sign_authentication(&self.authentication(role))
            .map(Auth)
    }
    /// Verify a peer proof against its role and advertised public key.
    /// # Errors
    /// Propagates signature rejection or primitive unavailability unchanged.
    pub fn verify(
        &self,
        role: ConnectionRole,
        public_key: &[u8; 32],
        auth: &Auth,
        crypto: &impl Cryptography,
    ) -> Result<(), DependencyError> {
        tracing::trace!(
            function = "Transcript::verify",
            "Verifying a role-bound peer proof"
        );
        crypto.verify_signature(public_key, self.authentication(role).as_bytes(), &auth.0)
    }
}
