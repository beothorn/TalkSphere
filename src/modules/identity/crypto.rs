//! Public-contract mapping from real Cryptography to Identity's injected dependency.
use super::{error::IdentityError, ports::Cryptography};
use crate::modules::cryptography::{
    api::Hashing,
    error::CryptoError,
    ports::Entropy,
    primitives::{signature, verification_key::VerificationKey},
    provider::Provider,
};
impl From<CryptoError> for IdentityError {
    /// Preserve distinguishable public failure categories without secret context.
    fn from(error: CryptoError) -> Self {
        tracing::trace!("Mapping Cryptography failure to Identity");
        match error {
            CryptoError::InvalidInput => Self::InvalidInput,
            CryptoError::AuthenticationFailed => Self::InvalidProof,
            CryptoError::AccessDenied => Self::AccessDenied,
            CryptoError::Unavailable => Self::Unavailable,
            CryptoError::ResourceLimit => Self::ResourceLimit,
        }
    }
}
impl<E: Entropy> Cryptography for Provider<E> {
    /// Hash bounded exact identity bytes using the real public provider.
    fn sha256(&self, input: &[u8]) -> Result<[u8; 32], IdentityError> {
        tracing::trace!("Serving Identity hash derivation");
        Hashing::sha256(self, input).map_err(Into::into)
    }
    /// Validate received public points without granting identity possession.
    fn validate_signing_key(&self, input: &[u8]) -> Result<(), IdentityError> {
        tracing::trace!("Serving Identity public-key validation");
        VerificationKey::parse(input)
            .map(|_| ())
            .map_err(Into::into)
    }
    /// Verify an exact proof with the provider's immutable resource ceiling.
    fn verify(&self, key: &[u8], message: &[u8], proof: &[u8]) -> Result<(), IdentityError> {
        tracing::trace!("Serving Identity exact proof verification");
        signature::verify(key, message, proof, self.maximum_bytes()).map_err(Into::into)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::api::Signatures;
    // Real adapter operations preserve exact proof bytes and every structured crypto failure.
    #[test]
    fn adapter_preserves_failure_classes_and_exact_proofs() {
        for (crypto, identity) in [
            (CryptoError::InvalidInput, IdentityError::InvalidInput),
            (
                CryptoError::AuthenticationFailed,
                IdentityError::InvalidProof,
            ),
            (CryptoError::AccessDenied, IdentityError::AccessDenied),
            (CryptoError::Unavailable, IdentityError::Unavailable),
            (CryptoError::ResourceLimit, IdentityError::ResourceLimit),
        ] {
            assert_eq!(IdentityError::from(crypto), identity);
        }
        let mut provider = Provider::system(148).unwrap();
        let pair = provider.generate_signing_key().unwrap();
        let proof = provider.sign(&pair.private_key, b"proof").unwrap();
        Cryptography::verify(&provider, &pair.public_key, b"proof", &proof).unwrap();
        assert_eq!(
            Cryptography::verify(&provider, &pair.public_key, b"wrong", &proof),
            Err(IdentityError::InvalidProof)
        );
        assert_eq!(
            Cryptography::sha256(&provider, &[0; 149]),
            Err(IdentityError::ResourceLimit)
        );
    }
}
