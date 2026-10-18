//! The Identity adapter grants Transport-specific signing authority only.

use super::DependencyError;

/// Exact binary identity; possession of these public bytes grants no authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdentityId(pub [u8; 32]);

/// The Transport projection of an Identity credential, excluding private keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicIdentity {
    pub id: IdentityId,
    pub signing_public_key: [u8; 32],
}

/// Exact, domain-separated bytes constructed only by Transport's handshake.
/// Construction and canonical encoding are implemented in step 7.
///
/// An ordinary caller cannot turn arbitrary peer bytes into a signing request.
/// ```compile_fail
/// use talksphere::modules::transport::ports::identity::AuthenticationMessage;
/// let message = AuthenticationMessage { bytes: vec![1, 2, 3] };
/// ```
pub struct AuthenticationMessage {
    pub(in crate::modules::transport) bytes: Vec<u8>,
}

impl AuthenticationMessage {
    /// Borrow the exact Transport-created signing message without exposing private credentials.
    pub fn as_bytes(&self) -> &[u8] {
        tracing::trace!(
            function = "AuthenticationMessage::as_bytes",
            "Borrowing canonical authentication input"
        );
        &self.bytes
    }
}

pub trait Identity: Send {
    /// Return a validated, existing credential; never silently create a new ID.
    fn public_identity(&self) -> Result<PublicIdentity, DependencyError>;
    /// Derive the binary ID by the canonical formula; this does not authenticate.
    fn identify(&self, signing_public_key: &[u8; 32]) -> Result<IdentityId, DependencyError>;
    /// Sign exactly these authentication bytes with the authorized local key.
    fn sign_authentication(
        &mut self,
        message: &AuthenticationMessage,
    ) -> Result<[u8; 64], DependencyError>;
}
