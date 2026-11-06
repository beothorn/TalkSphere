//! Generic encrypted claim redemption with exact issuer/owner binding and guarded plaintext.
use super::{
    Error,
    authority::{Action, Authority},
    claim::Portable,
    contract::Context,
};
use crate::modules::{
    cryptography::{
        api::{EncryptRequest, IdentityEncryption, RecipientCredential},
        config::ResourceLimits,
        ports::Entropy,
    },
    identity::{
        api::IdentityId,
        authority::{Capability, Decrypt},
        operations::EnvelopeFormat,
        ports::KeyStore,
        provider::{Identity, IdentityHandle},
        record::CredentialRecord,
    },
};
use zeroize::Zeroizing;
const DOMAIN: &[u8] = b"CR2SE-CLAIM-REDEEM-V1";
/// Exact complete generic envelope length for the fixed claim-redemption plaintext.
pub const ENVELOPE_BYTES: usize = DOMAIN.len() + 128 + 148;
/// Protected decryption dependency; only authenticated complete plaintext can be returned.
pub trait Decryptor {
    /// Decrypt this exact generic envelope without exposing persistent keys.
    /// # Errors
    /// Invalid recipient, failed authentication or unavailable/denied authority returns no plaintext.
    fn decrypt(&self, object: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error>;
}
/// Scoped adapter to a loaded protected Identity provider and Decrypt capability.
pub struct IdentityDecryptor<'a, S: KeyStore<Record = CredentialRecord>, E: Entropy> {
    /// Protected local Identity provider.
    pub identity: &'a Identity<S, E>,
    /// Existing local key handle, never peer supplied.
    pub handle: &'a IdentityHandle,
    /// Explicit local purpose-scoped Decrypt capability.
    pub capability: &'a Capability<Decrypt>,
}
impl<S: KeyStore<Record = CredentialRecord>, E: Entropy> Decryptor for IdentityDecryptor<'_, S, E> {
    // Identity authenticates the entire object before guarded plaintext can be observed.
    fn decrypt(&self, object: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
        tracing::trace!("Decrypting claim through protected Identity capability");
        self.identity
            .decrypt(
                self.handle,
                self.capability,
                EnvelopeFormat::GenericV1,
                object,
            )
            .map_err(|_| Error::Denied)
    }
}
impl Portable {
    /// Encrypt exact domain/issuer/owner/claimId/secret bytes to the verified original issuer credential.
    /// # Errors
    /// Wrong issuer credential, invalid encryption binding or entropy/resource failure returns no envelope.
    pub fn encrypt(
        &self,
        owner: IdentityId,
        recipient: &RecipientCredential,
        crypto: &mut impl IdentityEncryption,
    ) -> Result<Vec<u8>, Error> {
        tracing::trace!("Encrypting owner-bound claim redemption bearer material");
        if recipient.identity_id != self.issuer.0 {
            return Err(Error::Denied);
        }
        let mut plain = Zeroizing::new(Vec::with_capacity(DOMAIN.len() + 128));
        plain.extend_from_slice(DOMAIN);
        plain.extend_from_slice(&self.issuer.0);
        plain.extend_from_slice(&owner.0);
        plain.extend_from_slice(&self.id);
        plain.extend_from_slice(self.secret.as_ref());
        let limits = ResourceLimits::new(plain.len(), ENVELOPE_BYTES).map_err(|_| Error::Limit)?;
        crypto
            .encrypt(EncryptRequest {
                plaintext: &plain,
                recipient,
                limits,
            })
            .map_err(|_| Error::Denied)
    }
}
/// Validated guarded redemption; construction authenticates ciphertext and exact local context.
pub struct Redemption {
    pub(super) id: [u8; 32],
    pub(super) owner: IdentityId,
    pub(super) secret: Zeroizing<[u8; 32]>,
}
impl Redemption {
    /// Authenticate and decode the fixed-format redemption from a trusted peer context.
    /// # Errors
    /// Denied caller, wrong size/domain/issuer/owner, tampering or decryption failure returns no secret.
    pub fn decrypt(
        context: &Context<'_>,
        authority: &Authority,
        local: IdentityId,
        owner: IdentityId,
        object: &[u8],
        decryptor: &impl Decryptor,
    ) -> Result<Self, Error> {
        tracing::trace!("Authenticating complete owner-bound claim redemption");
        context.check(authority, local, Action::Control, object.len())?;
        if object.len() != ENVELOPE_BYTES || owner == local {
            return Err(Error::Invalid);
        }
        let plain = decryptor.decrypt(object)?;
        if plain.len() != DOMAIN.len() + 128 || &plain[..DOMAIN.len()] != DOMAIN {
            return Err(Error::Invalid);
        }
        let start = DOMAIN.len();
        if plain[start..start + 32] != local.0 || plain[start + 32..start + 64] != owner.0 {
            return Err(Error::Denied);
        }
        Ok(Self {
            id: plain[start + 64..start + 96]
                .try_into()
                .map_err(|_| Error::Invalid)?,
            owner,
            secret: Zeroizing::new(plain[start + 96..].try_into().map_err(|_| Error::Invalid)?),
        })
    }
    /// Return the public claim ID after authenticated plaintext validation.
    pub fn id(&self) -> [u8; 32] {
        tracing::trace!("Reading authenticated redemption claim identity");
        self.id
    }
    /// Return the authenticated new owner association without exposing bearer material.
    pub fn owner(&self) -> IdentityId {
        tracing::trace!("Reading authenticated redemption owner");
        self.owner
    }
}

impl Redemption {
    /// Return the SHA256 commitment for constant-time comparison without exposing the secret.
    pub fn commitment(&self) -> [u8; 32] {
        tracing::trace!("Computing guarded redemption commitment");
        use sha2::Digest;
        sha2::Sha256::digest(self.secret.as_ref()).into()
    }
}
#[cfg(test)]
#[path = "tests/redemption.rs"]
mod tests;
