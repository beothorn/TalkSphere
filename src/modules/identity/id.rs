//! Canonical identity derivation and public-key relation validation; neither establishes possession.
use super::{
    api::{IdentityId, PublicIdentity},
    error::IdentityError,
    ports::Cryptography,
};

/// Derive the exact version-one identifier through the injected Cryptography port.
/// # Errors
/// Rejects unsupported selectors or key lengths and propagates hash failures.
pub fn derive(
    crypto: &impl Cryptography,
    version: u8,
    algorithm: u8,
    key: &[u8],
) -> Result<IdentityId, IdentityError> {
    tracing::trace!("Deriving the canonical binary identity");
    if version != 1 || algorithm != 1 {
        return Err(IdentityError::Unsupported);
    }
    let key: &[u8; 32] = key.try_into().map_err(|_| IdentityError::InvalidInput)?;
    let mut input = [1; 34];
    input[2..].copy_from_slice(key);
    crypto.sha256(&input).map(IdentityId)
}

/// Validate a claimed public identity against its key and the canonical point profile.
/// # Errors
/// Rejects malformed IDs, unsupported selectors, mismatches, invalid keys and dependency failure.
/// Success proves only the key/ID relation, not authority, trust or a live connection.
pub fn validate(
    crypto: &impl Cryptography,
    version: u8,
    algorithm: u8,
    key: &[u8],
    claimed: &[u8],
) -> Result<PublicIdentity, IdentityError> {
    tracing::trace!("Validating a claimed public identity without granting possession");
    let expected: [u8; 32] = claimed
        .try_into()
        .map_err(|_| IdentityError::InvalidInput)?;
    let id = derive(crypto, version, algorithm, key)?;
    if id.0 != expected {
        return Err(IdentityError::IdentityMismatch);
    }
    crypto.validate_signing_key(key)?;
    let signing_public_key = key.try_into().map_err(|_| IdentityError::InvalidInput)?;
    Ok(PublicIdentity {
        id,
        signing_public_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{
        cryptography::provider::Provider,
        identity::test_support::{CryptoCall, ScriptedCrypto, TRANSPORT, vector},
    };
    use std::{cell::RefCell, collections::VecDeque};
    // Exact fixture IDs and prefix bytes match upstream, while relation validation rejects mismatches.
    #[test]
    fn canonical_ids_and_input_order_match_vectors() {
        let real = Provider::system(4096).unwrap();
        for role in ["initiator", "acceptor"] {
            let key = vector(TRANSPORT, &format!("{role} Ed25519 public key"));
            let expected = vector(TRANSPORT, &format!("{role} CR2SE ID"));
            let mut input = vec![1, 1];
            input.extend(&key);
            let script = ScriptedCrypto(RefCell::new(VecDeque::from([
                CryptoCall {
                    operation: "hash",
                    inputs: vec![input],
                    result: Ok(expected.clone()),
                },
                CryptoCall {
                    operation: "key",
                    inputs: vec![key.clone()],
                    result: Ok(vec![]),
                },
            ])));
            let identity = validate(&script, 1, 1, &key, &expected).unwrap();
            assert_eq!(derive(&real, 1, 1, &key).unwrap(), identity.id());
            assert_eq!(identity.signing_public_key().as_slice(), key);
            assert_eq!(
                validate(&real, 1, 1, &key, &[0; 32]),
                Err(IdentityError::IdentityMismatch)
            );
        }
    }
    // Derivation hashes bytes, but validation rejects weak keys even when their claimed ID is correct.
    #[test]
    fn malformed_claims_selectors_and_weak_keys_fail() {
        let real = Provider::system(4096).unwrap();
        for length in [0, 31, 33] {
            assert_eq!(
                derive(&real, 1, 1, &vec![0; length]),
                Err(IdentityError::InvalidInput)
            );
            assert_eq!(
                validate(&real, 1, 1, &[0; 32], &vec![0; length]),
                Err(IdentityError::InvalidInput)
            );
        }
        for (v, a) in [(0, 1), (2, 1), (1, 0), (1, 2)] {
            assert_eq!(
                derive(&real, v, a, &[0; 32]),
                Err(IdentityError::Unsupported)
            );
        }
        let mut weak = [0; 32];
        weak[0] = 1;
        let id = derive(&real, 1, 1, &weak).unwrap();
        assert!(validate(&real, 1, 1, &weak, &id.0).is_err());
        let script = ScriptedCrypto(RefCell::new(VecDeque::new()));
        assert_eq!(
            derive(&script, 1, 1, &[0; 32]),
            Err(IdentityError::Unavailable)
        );
    }
}
