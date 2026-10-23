//! Protected persistent identity material for authorized platform import/export and public derivation.
use super::{
    api::RecipientCredential,
    credential,
    error::CryptoError,
    ports::Entropy,
    primitives::keys,
    secrets::{Persistent, Secret, Signing},
};
use zeroize::Zeroizing;

/// Privileged platform destination; implementations must authorize before receiving any secret.
/// Staging must be erased on every failure; this is never a peer or ordinary result sink.
pub trait ProtectedKeySink {
    /// Check protected local authority before private bytes are borrowed.
    /// # Errors
    /// Returns denied/unavailable without receiving any private value.
    fn authorize(&self) -> Result<(), CryptoError>;
    /// Accept both private inputs as one guarded, complete association.
    /// Borrows last only for this call; any retained copy must remain protected.
    /// # Errors
    /// Reports storage failure without publishing a partial identity.
    fn store_pair(&mut self, signing: &[u8; 32], encryption: &[u8; 32]) -> Result<(), CryptoError>;
}

/// Independent signing and persistent encryption authority, with scoped erasure and no raw getters.
/// ```compile_fail
/// use talksphere::modules::cryptography::identity_keys::IdentityKeys;
/// fn leak(keys: IdentityKeys) { println!("{keys:?}"); }
/// ```
pub struct IdentityKeys {
    signing: Secret<Signing>,
    encryption: Secret<Persistent>,
}
impl IdentityKeys {
    /// Generate independent pairs and retain guarded ownership from first acquisition.
    /// # Errors
    /// Propagates entropy failure and releases any already-acquired key.
    pub fn generate(entropy: &mut impl Entropy) -> Result<Self, CryptoError> {
        tracing::trace!("Generating protected independent identity material");
        let signing = keys::signing_key(entropy)?.private_key;
        let encryption = keys::persistent_key(entropy)?.private_key;
        Ok(Self {
            signing,
            encryption,
        })
    }
    /// Import exact 32-byte protected private inputs from authorized local storage.
    /// Inputs are zeroizing owned buffers, not peer credentials or an export format.
    /// # Errors
    /// Rejects either wrong width and erases both staging buffers on every exit.
    pub fn import(
        signing: Zeroizing<Vec<u8>>,
        encryption: Zeroizing<Vec<u8>>,
    ) -> Result<Self, CryptoError> {
        tracing::trace!("Importing guarded identity material from a protected platform source");
        if signing.len() != 32 || encryption.len() != 32 {
            return Err(CryptoError::InvalidInput);
        }
        let mut s = Zeroizing::new([0; 32]);
        s.copy_from_slice(&signing);
        let mut e = Zeroizing::new([0; 32]);
        e.copy_from_slice(&encryption);
        Ok(Self {
            signing: Secret::new(s),
            encryption: Secret::new(e),
        })
    }
    /// Derive both public keys, the exact ID and deterministic encryption binding.
    /// # Errors
    /// Propagates canonical derivation/binding errors; returns no private material.
    pub fn credential(&self) -> Result<RecipientCredential, CryptoError> {
        tracing::trace!("Deriving a complete public credential from protected pairs");
        let signing_public_key = ed25519_dalek::SigningKey::from_bytes(&self.signing.bytes)
            .verifying_key()
            .to_bytes();
        let secret = x25519_dalek::StaticSecret::from(*self.encryption.bytes);
        let encryption_public_key = x25519_dalek::PublicKey::from(&secret).to_bytes();
        Ok(RecipientCredential {
            identity_id: credential::identity_id(&signing_public_key)?,
            signing_public_key,
            encryption_public_key,
            binding_signature: credential::bind(&self.signing, &encryption_public_key)?,
        })
    }
    /// Transfer borrowed private inputs only to an authorized protected platform sink.
    /// # Errors
    /// Checks sink authority before disclosure and propagates storage errors without retry.
    pub fn store(&self, sink: &mut impl ProtectedKeySink) -> Result<(), CryptoError> {
        tracing::trace!("Transferring identity material to an authorized protected sink");
        sink.authorize()?;
        sink.store_pair(&self.signing.bytes, &self.encryption.bytes)
    }
    /// Borrow purpose-specific signing authority for an already-authorized local adapter.
    pub fn signing(&self) -> &Secret<Signing> {
        tracing::trace!("Borrowing protected identity signing authority");
        &self.signing
    }
    /// Borrow purpose-specific persistent encryption authority for an authorized local adapter.
    pub fn encryption(&self) -> &Secret<Persistent> {
        tracing::trace!("Borrowing protected persistent decryption authority");
        &self.encryption
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::primitives::signature;
    use crate::modules::cryptography::test_support::{ScriptedEntropy, envelope, observe_releases};
    use std::{collections::VecDeque, sync::atomic::Ordering};
    struct Sink {
        permitted: bool,
        fail: bool,
        received: bool,
    }
    impl ProtectedKeySink for Sink {
        // Deny before the sink sees any fixture private input.
        fn authorize(&self) -> Result<(), CryptoError> {
            if self.permitted {
                Ok(())
            } else {
                Err(CryptoError::AccessDenied)
            }
        }
        // Observe a complete protected pair and simulate a storage failure without retaining it.
        fn store_pair(&mut self, s: &[u8; 32], e: &[u8; 32]) -> Result<(), CryptoError> {
            self.received = true;
            assert_ne!(s, e);
            if self.fail {
                Err(CryptoError::Unavailable)
            } else {
                Ok(())
            }
        }
    }
    // Known private fixtures reconstruct the exact public association; sink denial precedes disclosure.
    #[test]
    fn protected_transfer_and_public_derivation_preserve_exact_pairs() {
        let keys = IdentityKeys::import(
            envelope("recipient Ed25519 seed").into(),
            envelope("recipient raw persistent X25519 private-key input").into(),
        )
        .unwrap();
        let credential = keys.credential().unwrap();
        assert_eq!(
            credential.encryption_public_key.as_slice(),
            envelope("recipient persistent X25519 public key")
        );
        assert_eq!(
            credential.binding_signature.as_slice(),
            envelope("encryption-key binding signature")
        );
        let proof = signature::sign(keys.signing(), b"test", 4).unwrap();
        signature::verify(&credential.signing_public_key, b"test", &proof, 4).unwrap();
        let _persistent = keys.encryption();
        let mut sink = Sink {
            permitted: false,
            fail: false,
            received: false,
        };
        assert_eq!(keys.store(&mut sink), Err(CryptoError::AccessDenied));
        assert!(!sink.received);
        sink.permitted = true;
        keys.store(&mut sink).unwrap();
        assert!(sink.received);
        sink.fail = true;
        assert_eq!(keys.store(&mut sink), Err(CryptoError::Unavailable));
        for (s, e) in [(31, 32), (32, 33)] {
            assert!(IdentityKeys::import(vec![1; s].into(), vec![2; e].into()).is_err());
        }
    }
    // Second-pair failure erases the first key; successful generation keeps both until caller drop.
    #[test]
    fn key_acquisition_is_guarded_across_dependency_failure() {
        let (result, released) = observe_releases(|| {
            IdentityKeys::generate(&mut ScriptedEntropy(VecDeque::from([
                Ok(vec![1; 32]),
                Err(CryptoError::Unavailable),
            ])))
        });
        assert!(result.is_err());
        assert_eq!(released.load(Ordering::SeqCst), 1);
        let (keys, released) = observe_releases(|| {
            IdentityKeys::generate(&mut ScriptedEntropy(VecDeque::from([
                Ok(vec![1; 32]),
                Ok(vec![2; 32]),
            ])))
            .unwrap()
        });
        assert_eq!(released.load(Ordering::SeqCst), 0);
        drop(keys);
        assert_eq!(released.load(Ordering::SeqCst), 2);
    }
}
