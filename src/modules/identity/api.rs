//! Typed public identity and contact snapshots; possession of public bytes grants no private authority.
use super::error::IdentityError;
use std::net::SocketAddr;

/// Canonical binary identifier, independent of its display spelling or endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdentityId(pub [u8; 32]);

/// Validated ID/key relationship; construction belongs to Identity validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicIdentity {
    pub(super) id: IdentityId,
    pub(super) signing_public_key: [u8; 32],
}
impl PublicIdentity {
    /// Read the canonical binary ID without granting proof of possession.
    pub fn id(&self) -> IdentityId {
        tracing::trace!("Reading a validated public identity ID");
        self.id
    }
    /// Read the validated signing key without granting trust or private authority.
    pub fn signing_public_key(&self) -> [u8; 32] {
        tracing::trace!("Reading a validated public signing key");
        self.signing_public_key
    }
}

/// Public complete credential; validating a received credential remains mandatory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicCredential {
    /// Validated public signing identity.
    pub identity: PublicIdentity,
    /// Persistent X25519 public key, distinct from the signing key.
    pub encryption_public_key: [u8; 32],
    /// Signature over the specified encryption-key binding bytes.
    pub binding_signature: [u8; 64],
}

/// Immutable configured contact hint; it does not assert reachability or authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Contact {
    /// ID required when the eventual connection authenticates.
    pub expected_id: IdentityId,
    /// Optional bootstrap IP/port; an ID alone cannot locate an unknown network.
    pub endpoint: Option<SocketAddr>,
}

/// Version of the in-process contract, independent of wire versions.
pub const CONTRACT_VERSION: u16 = 1;

/// Check the local binding version before a provider performs effects.
/// # Errors
/// Rejects every version other than one without fallback.
pub fn validate_contract(version: u16) -> Result<(), IdentityError> {
    tracing::trace!("Checking Identity local contract compatibility");
    if version != CONTRACT_VERSION {
        return Err(IdentityError::Unsupported);
    }
    Ok(())
}

/// Public lifecycle contract; implementations own opaque handles and local authorization.
pub trait IdentityProvider {
    /// Scoped handle without a required byte or diagnostic representation.
    type Handle;
    /// Trusted local administrative authority, never taken from peer payloads.
    type Administration;
    /// Create a durable complete identity for authorized administration.
    /// # Errors
    /// The default explicitly fails until durable creation is implemented.
    fn create(
        &mut self,
        _authority: &Self::Administration,
    ) -> Result<(Self::Handle, PublicCredential), IdentityError> {
        tracing::trace!("Rejecting unavailable Identity creation");
        Err(IdentityError::Unavailable)
    }
    /// Load existing authority without silently replacing missing keys.
    /// # Errors
    /// The default explicitly fails until protected loading is implemented.
    fn load(
        &mut self,
        _authority: &Self::Administration,
    ) -> Result<(Self::Handle, PublicCredential), IdentityError> {
        tracing::trace!("Rejecting unavailable Identity loading");
        Err(IdentityError::Unavailable)
    }
}
