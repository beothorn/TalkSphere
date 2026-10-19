//! Exact 124-byte identity-envelope context and domain-separated key derivation.

use super::super::{
    error::CryptoError,
    primitives::{hash, kdf},
    secrets::{Content, Secret, Shared},
};
use super::header::{CONTEXT_BYTES, Header};
const SALT_PREFIX: &[u8] = b"CR2SE-IDENTITY-ENCRYPTION-SALT-V1\0";
const KEY_PREFIX: &[u8] = b"CR2SE-IDENTITY-ENCRYPTION-KEY-V1\0";

/// Extract suite, recipient ID/key, ephemeral key and nonce in canonical order.
/// # Errors
/// Propagates invalid header metadata; returns exactly 124 context bytes.
pub fn context(header: &Header) -> Result<[u8; CONTEXT_BYTES], CryptoError> {
    tracing::trace!("Constructing the exact identity-encryption KDF context");
    header.encode()?[..CONTEXT_BYTES]
        .try_into()
        .map_err(|_| CryptoError::InvalidInput)
}

/// Hash the salt prefix, explicit zero separator and exact supported context.
/// # Errors
/// Rejects unsupported suite bytes without deriving material for another format.
pub fn salt(context: &[u8; CONTEXT_BYTES]) -> Result<[u8; 32], CryptoError> {
    tracing::trace!("Deriving the identity-encryption salt");
    if context[..4] != [1; 4] {
        return Err(CryptoError::InvalidInput);
    }
    let mut bytes = [0; SALT_PREFIX.len() + CONTEXT_BYTES];
    bytes[..SALT_PREFIX.len()].copy_from_slice(SALT_PREFIX);
    bytes[SALT_PREFIX.len()..].copy_from_slice(context);
    hash::sha256(&bytes, bytes.len())
}

/// Consume shared material and derive a Content key using exact envelope labels.
/// # Errors
/// Rejects unsupported context suites or propagates derivation failure; secrets erase on exit.
pub fn derive(
    shared: Secret<Shared>,
    context: &[u8; CONTEXT_BYTES],
) -> Result<Secret<Content>, CryptoError> {
    tracing::trace!("Deriving the generic identity-envelope encryption key");
    let prk = kdf::extract(&salt(context)?, shared, 32)?;
    let mut info = [0; KEY_PREFIX.len() + CONTEXT_BYTES];
    info[..KEY_PREFIX.len()].copy_from_slice(KEY_PREFIX);
    info[KEY_PREFIX.len()..].copy_from_slice(context);
    kdf::expand_key(&prk, &info, info.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::envelope;
    use zeroize::Zeroizing;
    // The fixed vector independently pins context, salt and final key; every context byte is bound.
    #[test]
    fn exact_context_and_context_mutations() {
        let header = Header::decode(&envelope("132-byte header")).unwrap();
        let context = context(&header).unwrap();
        assert_eq!(context.as_slice(), envelope("KDF context"));
        assert_eq!(salt(&context).unwrap().as_slice(), envelope("HKDF salt"));
        let key = derive(shared(), &context).unwrap();
        assert_eq!(key.bytes.as_slice(), envelope("derived encryption key"));
        for index in 0..CONTEXT_BYTES {
            let mut changed = context;
            changed[index] ^= 1;
            let result = derive(shared(), &changed);
            if index < 4 {
                assert!(matches!(result, Err(CryptoError::InvalidInput)));
            } else {
                assert_ne!(result.unwrap().bytes, key.bytes);
            }
        }
    }
    // Import only public fixture material into test-owned shared-secret authority.
    fn shared() -> Secret<Shared> {
        Secret::new(Zeroizing::new(
            envelope("X25519 shared secret").try_into().unwrap(),
        ))
    }
}
