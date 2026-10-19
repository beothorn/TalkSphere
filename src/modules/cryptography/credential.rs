//! Exact persistent encryption-key binding; validation grants no trust or rotation authority.

use super::{
    api::RecipientCredential,
    error::CryptoError,
    primitives::{hash, signature},
    secrets::{Secret, Signing},
};
const PREFIX: &[u8] = b"CR2SE-ENCRYPTION-KEY-V1";
/// Exact binding message size: ASCII prefix without terminator, followed by 32 key bytes.
pub const BINDING_BYTES: usize = PREFIX.len() + 32;

/// Verified public recipient values, constructible only through binding validation.
pub struct ValidatedRecipient {
    identity_id: [u8; 32],
    encryption_public_key: [u8; 32],
}
impl ValidatedRecipient {
    /// Return the verified ID digest; it does not establish trust in the identity.
    pub fn identity_id(&self) -> [u8; 32] {
        tracing::trace!("Reading a validated recipient ID");
        self.identity_id
    }
    /// Return the persistent public key authorized by the verified binding.
    pub fn encryption_public_key(&self) -> [u8; 32] {
        tracing::trace!("Reading a bound recipient encryption key");
        self.encryption_public_key
    }
}

/// Copy exact-size untrusted credential fields without implying successful validation.
/// # Errors
/// Returns `InvalidInput` for any wrong key/ID/signature size.
pub fn parse(
    identity_id: &[u8],
    signing_public_key: &[u8],
    encryption_public_key: &[u8],
    binding_signature: &[u8],
) -> Result<RecipientCredential, CryptoError> {
    tracing::trace!("Parsing fixed-size public credential fields");
    Ok(RecipientCredential {
        identity_id: identity_id
            .try_into()
            .map_err(|_| CryptoError::InvalidInput)?,
        signing_public_key: signing_public_key
            .try_into()
            .map_err(|_| CryptoError::InvalidInput)?,
        encryption_public_key: encryption_public_key
            .try_into()
            .map_err(|_| CryptoError::InvalidInput)?,
        binding_signature: binding_signature
            .try_into()
            .map_err(|_| CryptoError::InvalidInput)?,
    })
}

/// Derive SHA-256(0x01 || 0x01 || Ed25519 public key) without an Identity runtime.
/// # Errors
/// Returns `InvalidInput` unless the public key is exactly 32 bytes; this formula alone is not proof.
pub fn identity_id(public_key: &[u8]) -> Result<[u8; 32], CryptoError> {
    tracing::trace!("Deriving the version-one public identity digest");
    let key: &[u8; 32] = public_key
        .try_into()
        .map_err(|_| CryptoError::InvalidInput)?;
    let mut bytes = [1; 34];
    bytes[2..].copy_from_slice(key);
    hash::sha256(&bytes, bytes.len())
}

/// Construct exact binding bytes without a zero separator or length prefix.
/// # Errors
/// Returns `InvalidInput` unless the encryption public key is exactly 32 bytes.
pub fn binding_bytes(public_key: &[u8]) -> Result<[u8; BINDING_BYTES], CryptoError> {
    tracing::trace!("Constructing canonical encryption-key binding bytes");
    let key: &[u8; 32] = public_key
        .try_into()
        .map_err(|_| CryptoError::InvalidInput)?;
    let mut bytes = [0; BINDING_BYTES];
    bytes[..PREFIX.len()].copy_from_slice(PREFIX);
    bytes[PREFIX.len()..].copy_from_slice(key);
    Ok(bytes)
}

/// Sign the exact persistent-key binding using authorized local signing authority.
/// # Errors
/// Rejects malformed encryption public keys or propagates signing failures.
pub fn bind(key: &Secret<Signing>, public_key: &[u8]) -> Result<[u8; 64], CryptoError> {
    tracing::trace!("Authorizing a persistent encryption public key");
    signature::sign(key, &binding_bytes(public_key)?, BINDING_BYTES)
}

/// Verify the claimed ID first, then its ordinary Ed25519 encryption-key binding.
/// # Errors
/// Rejects mismatched IDs or invalid bindings; strict weak-point policy follows signature verification.
pub fn validate(credential: &RecipientCredential) -> Result<ValidatedRecipient, CryptoError> {
    tracing::trace!("Validating a recipient identity and persistent-key binding");
    if identity_id(&credential.signing_public_key)? != credential.identity_id {
        return Err(CryptoError::AuthenticationFailed);
    }
    signature::verify(
        &credential.signing_public_key,
        &binding_bytes(&credential.encryption_public_key)?,
        &credential.binding_signature,
        BINDING_BYTES,
    )?;
    Ok(ValidatedRecipient {
        identity_id: credential.identity_id,
        encryption_public_key: credential.encryption_public_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::envelope;
    use zeroize::Zeroizing;
    // CR-03 matches exact binding bytes and rejects every changed credential field byte.
    #[test]
    fn fixed_binding_and_credential_mutations() {
        let credential = parse(
            &envelope("recipient CR2SE ID"),
            &envelope("recipient Ed25519 public key"),
            &envelope("recipient persistent X25519 public key"),
            &envelope("encryption-key binding signature"),
        )
        .unwrap();
        let key = Secret::new(Zeroizing::new(
            envelope("recipient Ed25519 seed").try_into().unwrap(),
        ));
        assert_eq!(
            bind(&key, &credential.encryption_public_key).unwrap(),
            credential.binding_signature
        );
        let verified = validate(&credential).unwrap();
        assert_eq!(verified.identity_id(), credential.identity_id);
        assert_eq!(
            verified.encryption_public_key(),
            credential.encryption_public_key
        );
        for field in 0..4 {
            for byte in 0..if field == 3 { 64 } else { 32 } {
                let mut changed = credential;
                match field {
                    0 => changed.identity_id[byte] ^= 1,
                    1 => changed.signing_public_key[byte] ^= 1,
                    2 => changed.encryption_public_key[byte] ^= 1,
                    _ => changed.binding_signature[byte] ^= 1,
                }
                assert!(validate(&changed).is_err());
            }
        }
        for length in [63, 65] {
            assert!(parse(&[0; 32], &[0; 32], &[0; 32], &vec![0; length]).is_err());
        }
        assert!(parse(&[0; 31], &[0; 32], &[0; 32], &[0; 64]).is_err());
        assert!(parse(&[0; 32], &[0; 31], &[0; 32], &[0; 64]).is_err());
        assert!(parse(&[0; 32], &[0; 32], &[0; 31], &[0; 64]).is_err());
        assert!(identity_id(&[0; 31]).is_err());
        assert!(binding_bytes(&[0; 33]).is_err());
    }
}
