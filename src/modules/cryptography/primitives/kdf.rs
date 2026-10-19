//! Exact HKDF-SHA-256 derivation with protected PRKs and purpose-specific outputs.

use super::super::{
    error::CryptoError,
    secrets::{Prk, Secret, Shared},
};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

/// Extract a PRK from an owned X25519 shared secret and exact caller-defined salt.
/// # Errors
/// Rejects salt above the local byte budget; consumed shared material is released.
pub fn extract(
    salt: &[u8],
    shared: Secret<Shared>,
    maximum_bytes: usize,
) -> Result<Secret<Prk>, CryptoError> {
    tracing::trace!("Extracting a protected purpose-specific PRK");
    if salt.len() > maximum_bytes {
        return Err(CryptoError::ResourceLimit);
    }
    let (mut prk, _backend) = Hkdf::<Sha256>::extract(Some(salt), shared.bytes.as_slice());
    let mut bytes = Zeroizing::new([0; 32]);
    bytes.copy_from_slice(&prk);
    prk.as_mut_slice().zeroize();
    Ok(Secret::new(bytes))
}

/// Expand exact info into a protected 32-byte key whose purpose is selected by its construction.
/// # Errors
/// Returns `ResourceLimit` for excessive info or invalid backend output length.
pub fn expand_key<P>(
    prk: &Secret<Prk>,
    info: &[u8],
    maximum_bytes: usize,
) -> Result<Secret<P>, CryptoError> {
    tracing::trace!("Expanding a protected 32-byte construction key");
    let mut bytes = Zeroizing::new([0; 32]);
    expand(prk, info, bytes.as_mut(), maximum_bytes)?;
    Ok(Secret::new(bytes))
}

/// Expand exact info into the public 16-byte Transport nonce prefix.
/// # Errors
/// Returns `ResourceLimit` for excessive info; output is never a secret key.
pub fn expand_nonce_prefix(
    prk: &Secret<Prk>,
    info: &[u8],
    maximum_bytes: usize,
) -> Result<[u8; 16], CryptoError> {
    tracing::trace!("Expanding a public directional nonce prefix");
    let mut bytes = [0; 16];
    expand(prk, info, &mut bytes, maximum_bytes)?;
    Ok(bytes)
}

// Validate RFC5869's output bound before delegating exact expansion to the established backend.
fn expand(
    prk: &Secret<Prk>,
    info: &[u8],
    output: &mut [u8],
    maximum_bytes: usize,
) -> Result<(), CryptoError> {
    tracing::trace!("Validating HKDF expansion bounds");
    if info.len() > maximum_bytes || output.len() > 255 * 32 {
        return Err(CryptoError::ResourceLimit);
    }
    let backend =
        Hkdf::<Sha256>::from_prk(prk.bytes.as_slice()).map_err(|_| CryptoError::InvalidInput)?;
    backend
        .expand(info, output)
        .map_err(|_| CryptoError::ResourceLimit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::{
        secrets::Content,
        test_support::{envelope, hex},
    };
    // Pinned envelope extraction/expansion checks exact separators and intermediate bytes.
    #[test]
    fn pinned_derivation_matches_every_intermediate() {
        let shared = Secret::new(Zeroizing::new(
            envelope("X25519 shared secret").try_into().unwrap(),
        ));
        let prk = extract(&envelope("HKDF salt"), shared, 32).unwrap();
        assert_eq!(prk.bytes.as_slice(), envelope("HKDF extract PRK"));
        let mut info = b"CR2SE-IDENTITY-ENCRYPTION-KEY-V1\0".to_vec();
        info.extend(envelope("KDF context"));
        let key = expand_key::<Content>(&prk, &info, info.len()).unwrap();
        assert_eq!(key.bytes.as_slice(), envelope("derived encryption key"));
        info[0] ^= 1;
        assert_ne!(
            expand_key::<Content>(&prk, &info, info.len())
                .unwrap()
                .bytes,
            key.bytes
        );
        assert!(matches!(
            expand_key::<Content>(&prk, &info, 0),
            Err(CryptoError::ResourceLimit)
        ));
        assert!(matches!(
            extract(&[0; 33], Secret::new(Zeroizing::new([1; 32])), 32),
            Err(CryptoError::ResourceLimit)
        ));
    }
    // RFC5869 case 1 expansion is independent of CR2SE; zero/exact maximum/one-over are checked.
    #[test]
    fn rfc5869_expand_and_boundaries() {
        let prk = Secret::new(Zeroizing::new(
            hex("077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5")
                .try_into()
                .unwrap(),
        ));
        let info = hex("f0f1f2f3f4f5f6f7f8f9");
        let mut output = [0; 42];
        expand(&prk, &info, &mut output, 10).unwrap();
        assert_eq!(
            output.as_slice(),
            hex(
                "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
            )
        );
        assert_eq!(
            expand_nonce_prefix(&prk, &info, 10).unwrap().as_slice(),
            &output[..16]
        );
        assert!(expand(&prk, &[], &mut [], 0).is_ok());
        assert!(expand(&prk, &[], &mut vec![0; 8160], 0).is_ok());
        assert_eq!(
            expand(&prk, &[], &mut vec![0; 8161], 0),
            Err(CryptoError::ResourceLimit)
        );
    }
}
