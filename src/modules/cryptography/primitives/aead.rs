//! Bounded XChaCha20-Poly1305 with private zeroizing staging until authentication.

use super::super::{
    error::CryptoError,
    secrets::{Content, Secret},
};
use chacha20poly1305::{AeadInPlace, KeyInit, Tag, XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

/// Seal exact plaintext and associated data using a construction-assigned unique nonce.
/// The caller must never reuse the 24-byte nonce with this key.
/// # Errors
/// Rejects invalid nonce length, excessive plaintext/AAD/sealed size, allocation failure
/// or backend failure without returning a partial encrypted unit.
pub fn seal(
    key: &Secret<Content>,
    nonce: &[u8],
    plaintext: &[u8],
    aad: &[u8],
    maximum_bytes: usize,
) -> Result<Vec<u8>, CryptoError> {
    tracing::trace!("Sealing a bounded authenticated unit");
    let nonce = nonce_bytes(nonce)?;
    let total = plaintext
        .len()
        .checked_add(16)
        .ok_or(CryptoError::ResourceLimit)?;
    if total > maximum_bytes || aad.len() > maximum_bytes {
        return Err(CryptoError::ResourceLimit);
    }
    let mut buffer = staging(plaintext, total)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.bytes.as_slice())
        .map_err(|_| CryptoError::InvalidInput)?;
    let tag = cipher
        .encrypt_in_place_detached(XNonce::from_slice(nonce), aad, &mut buffer)
        .map_err(|_| CryptoError::InvalidInput)?;
    buffer.extend_from_slice(&tag);
    Ok(std::mem::take(&mut *buffer))
}

/// Authenticate a complete unit before releasing owned plaintext to the authorized caller.
/// Returned plaintext remains untrusted content and must not be logged by callers.
/// # Errors
/// Rejects invalid nonce/tag lengths, limits, allocation failure or authentication failure;
/// failure staging is erased and no provisional plaintext escapes.
pub fn open(
    key: &Secret<Content>,
    nonce: &[u8],
    sealed: &[u8],
    aad: &[u8],
    maximum_bytes: usize,
) -> Result<Vec<u8>, CryptoError> {
    tracing::trace!("Authenticating a bounded encrypted unit before plaintext release");
    let nonce = nonce_bytes(nonce)?;
    if sealed.len() > maximum_bytes || aad.len() > maximum_bytes {
        return Err(CryptoError::ResourceLimit);
    }
    let length = sealed
        .len()
        .checked_sub(16)
        .ok_or(CryptoError::InvalidInput)?;
    let mut buffer = staging(&sealed[..length], length)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.bytes.as_slice())
        .map_err(|_| CryptoError::InvalidInput)?;
    cipher
        .decrypt_in_place_detached(
            XNonce::from_slice(nonce),
            aad,
            &mut buffer,
            Tag::from_slice(&sealed[length..]),
        )
        .map_err(|_| CryptoError::AuthenticationFailed)?;
    Ok(std::mem::take(&mut *buffer))
}

// Reject incorrect nonce sizes before a backend API that requires exactly 24 bytes.
fn nonce_bytes(nonce: &[u8]) -> Result<&[u8; 24], CryptoError> {
    tracing::trace!("Validating an XChaCha20 nonce size");
    nonce.try_into().map_err(|_| CryptoError::InvalidInput)
}

// Allocate only a prevalidated capacity and protect any copied plaintext until success.
fn staging(input: &[u8], capacity: usize) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    tracing::trace!("Allocating bounded private authentication staging");
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| CryptoError::ResourceLimit)?;
    bytes.extend_from_slice(input);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::envelope;
    // Pinned encryption and decryption compare independent ciphertext and exercise all tampering.
    #[test]
    fn pinned_aead_and_invalid_units() {
        let key = Secret::new(Zeroizing::new(
            envelope("derived encryption key").try_into().unwrap(),
        ));
        let nonce = envelope("nonce");
        let aad = envelope("132-byte header");
        let plain = envelope("plaintext ASCII(\"CR2SE identity encryption test\")");
        let sealed = envelope("sealed ciphertext plus tag (46 bytes)");
        assert_eq!(seal(&key, &nonce, &plain, &aad, 1024).unwrap(), sealed);
        assert_eq!(open(&key, &nonce, &sealed, &aad, 1024).unwrap(), plain);
        for i in 0..sealed.len() {
            let mut changed = sealed.clone();
            changed[i] ^= 1;
            assert_eq!(
                open(&key, &nonce, &changed, &aad, 1024),
                Err(CryptoError::AuthenticationFailed)
            );
        }
        for i in 0..aad.len() {
            let mut changed = aad.clone();
            changed[i] ^= 1;
            assert!(open(&key, &nonce, &sealed, &changed, 1024).is_err());
        }
        let wrong = Secret::new(Zeroizing::new([1; 32]));
        assert!(open(&wrong, &nonce, &sealed, &aad, 1024).is_err());
        assert!(open(&key, &[0; 24], &sealed, &aad, 1024).is_err());
        assert!(open(&key, &nonce, &sealed[..15], &[], 1024).is_err());
        assert!(seal(&key, &nonce[..23], &plain, &aad, 1024).is_err());
        assert!(open(&key, &nonce, &sealed, &aad, 0).is_err());
        assert!(seal(&key, &nonce, &plain, &aad, 0).is_err());
        let empty = seal(&key, &[2; 24], &[], &[], 16).unwrap();
        assert_eq!(empty.len(), 16);
        assert!(open(&key, &[2; 24], &empty, &[], 16).unwrap().is_empty());
    }
}
