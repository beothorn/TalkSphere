//! Transport's empty-plaintext XChaCha20-Poly1305 tags over visible frame bytes.

use super::super::{
    error::CryptoError,
    secrets::{Integrity, Secret},
};
use chacha20poly1305::{AeadInPlace, KeyInit, Tag, XChaCha20Poly1305, XNonce};

/// Authenticate exact visible header/payload bytes with a directional integrity key.
/// Transport must assign a unique 24-byte nonce using its prefix and sequence.
/// # Errors
/// Rejects excessive AAD before work or returns a structured backend error.
pub fn tag(
    key: &Secret<Integrity>,
    nonce: &[u8; 24],
    aad: &[u8],
    maximum_bytes: usize,
) -> Result<[u8; 16], CryptoError> {
    tracing::trace!("Authenticating visible Transport frame bytes");
    if aad.len() > maximum_bytes {
        return Err(CryptoError::ResourceLimit);
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key.bytes.as_slice())
        .map_err(|_| CryptoError::InvalidInput)?;
    cipher
        .encrypt_in_place_detached(XNonce::from_slice(nonce), aad, &mut [])
        .map(Into::into)
        .map_err(|_| CryptoError::InvalidInput)
}

/// Verify a tag using the backend's constant-time authenticated-decryption operation.
/// # Errors
/// Returns `ResourceLimit` for excessive AAD or `AuthenticationFailed` for a wrong tag.
pub fn verify_tag(
    key: &Secret<Integrity>,
    nonce: &[u8; 24],
    aad: &[u8],
    tag: &[u8; 16],
    maximum_bytes: usize,
) -> Result<(), CryptoError> {
    tracing::trace!("Verifying a Transport integrity tag");
    if aad.len() > maximum_bytes {
        return Err(CryptoError::ResourceLimit);
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key.bytes.as_slice())
        .map_err(|_| CryptoError::InvalidInput)?;
    cipher
        .decrypt_in_place_detached(
            XNonce::from_slice(nonce),
            aad,
            &mut [],
            Tag::from_slice(tag),
        )
        .map_err(|_| CryptoError::AuthenticationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::transport;
    use zeroize::Zeroizing;
    // The first protected wire frame matches the pinned tag while its payload stays visible.
    #[test]
    fn fixed_frame_integrity_rejects_direction_nonce_and_byte_changes() {
        let key = Secret::new(Zeroizing::new(
            transport("initiator-to-acceptor key").try_into().unwrap(),
        ));
        let nonce = transport("nonce prefix || uint64_be(0)")
            .try_into()
            .unwrap();
        let expected = transport("integrity tag").try_into().unwrap();
        let mut aad = transport("header");
        aad.extend(transport("payload"));
        assert_eq!(tag(&key, &nonce, &aad, aad.len()).unwrap(), expected);
        verify_tag(&key, &nonce, &aad, &expected, aad.len()).unwrap();
        for i in 0..aad.len() {
            aad[i] ^= 1;
            assert!(verify_tag(&key, &nonce, &aad, &expected, aad.len()).is_err());
            aad[i] ^= 1;
        }
        let reverse = Secret::new(Zeroizing::new(
            transport("acceptor-to-initiator key").try_into().unwrap(),
        ));
        assert!(verify_tag(&reverse, &nonce, &aad, &expected, aad.len()).is_err());
        let mut wrong_nonce = nonce;
        wrong_nonce[23] ^= 1;
        assert!(verify_tag(&key, &wrong_nonce, &aad, &expected, aad.len()).is_err());
        assert_eq!(tag(&key, &nonce, &aad, 0), Err(CryptoError::ResourceLimit));
        assert_eq!(
            verify_tag(&key, &nonce, &aad, &expected, 0),
            Err(CryptoError::ResourceLimit)
        );
    }
}
