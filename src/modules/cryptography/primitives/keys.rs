//! Independent key generation through secure entropy, without persistent storage.

use super::super::{
    api::KeyPair,
    error::CryptoError,
    ports::Entropy,
    random::random_bytes,
    secrets::{Ephemeral, Persistent, Secret, Signing},
};

/// Generate an Ed25519 pair from fresh independent secure entropy.
/// # Errors
/// Propagates entropy failures; partial temporary material is erased on drop.
pub fn signing_key(entropy: &mut impl Entropy) -> Result<KeyPair<Secret<Signing>>, CryptoError> {
    tracing::trace!("Generating an independent Ed25519 signing pair");
    let bytes = random_bytes(entropy)?;
    let backend = ed25519_dalek::SigningKey::from_bytes(&bytes);
    Ok(KeyPair {
        public_key: backend.verifying_key().to_bytes(),
        private_key: Secret::new(bytes),
    })
}

/// Generate a persistent X25519 pair; Identity must persist it separately.
/// # Errors
/// Propagates entropy failures without converting or reusing an Ed25519 key.
pub fn persistent_key(
    entropy: &mut impl Entropy,
) -> Result<KeyPair<Secret<Persistent>>, CryptoError> {
    tracing::trace!("Generating an independent persistent X25519 pair");
    agreement_key(entropy)
}

/// Generate single-use ephemeral X25519 material for one construction.
/// # Errors
/// Propagates entropy failure with no deterministic fallback.
pub fn ephemeral_key(
    entropy: &mut impl Entropy,
) -> Result<KeyPair<Secret<Ephemeral>>, CryptoError> {
    tracing::trace!("Generating fresh ephemeral X25519 material");
    agreement_key(entropy)
}

// Share only the standard X25519 public-key derivation; distinct purposes remain typed.
fn agreement_key<P>(entropy: &mut impl Entropy) -> Result<KeyPair<Secret<P>>, CryptoError> {
    tracing::trace!("Deriving a canonical X25519 public key");
    let bytes = random_bytes(entropy)?;
    let backend = x25519_dalek::StaticSecret::from(*bytes);
    let public_key = x25519_dalek::PublicKey::from(&backend).to_bytes();
    Ok(KeyPair {
        public_key,
        private_key: Secret::new(bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::{ReleaseProbe, ScriptedEntropy, envelope};
    use std::{
        collections::VecDeque,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    // Independent requests reproduce pinned public keys without converting signing keys.
    #[test]
    fn generation_matches_fixed_keys_and_consumes_independent_entropy() {
        let mut entropy = ScriptedEntropy(VecDeque::from([
            Ok(envelope("recipient Ed25519 seed")),
            Ok(envelope(
                "recipient raw persistent X25519 private-key input",
            )),
            Ok(envelope("raw ephemeral X25519 private-key input")),
        ]));
        assert_eq!(
            signing_key(&mut entropy).unwrap().public_key.as_slice(),
            envelope("recipient Ed25519 public key")
        );
        assert_eq!(
            persistent_key(&mut entropy).unwrap().public_key.as_slice(),
            envelope("recipient persistent X25519 public key")
        );
        assert_eq!(
            ephemeral_key(&mut entropy).unwrap().public_key.as_slice(),
            envelope("ephemeral X25519 public key")
        );
        assert!(entropy.0.is_empty());
        assert!(matches!(
            signing_key(&mut entropy),
            Err(CryptoError::Unavailable)
        ));
        assert!(matches!(
            persistent_key(&mut entropy),
            Err(CryptoError::Unavailable)
        ));
    }

    // A local caller composing two generations drops its first capability if the second fails.
    #[test]
    fn failed_second_generation_releases_first_capability() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut entropy = ScriptedEntropy(VecDeque::from([
            Ok(vec![1; 32]),
            Err(CryptoError::Unavailable),
        ]));
        let result: Result<(), CryptoError> = (|| {
            let mut first = signing_key(&mut entropy)?;
            first.private_key.release = Some(ReleaseProbe(count.clone()));
            let _second = persistent_key(&mut entropy)?;
            Ok(())
        })();
        assert_eq!(result, Err(CryptoError::Unavailable));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
