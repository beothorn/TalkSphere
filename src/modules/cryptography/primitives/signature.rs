//! Ordinary Ed25519 over exact bounded bytes, with explicit strict verification policy.

use super::super::{
    error::CryptoError,
    secrets::{Secret, Signing},
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

/// Sign exact bytes with owned local authority, without hashing or serializing them first.
/// # Errors
/// Returns `ResourceLimit` for a message above the local byte budget.
pub fn sign(
    key: &Secret<Signing>,
    message: &[u8],
    maximum_bytes: usize,
) -> Result<[u8; 64], CryptoError> {
    tracing::trace!("Signing exact canonical bytes with local authority");
    if message.len() > maximum_bytes {
        return Err(CryptoError::ResourceLimit);
    }
    Ok(SigningKey::from_bytes(&key.bytes).sign(message).to_bytes())
}

/// Verify a 32-byte key and 64-byte ordinary Ed25519 signature over exact bytes.
/// Strict weak-point rejection is a documented local policy pending upstream clarification.
/// # Errors
/// Returns `InvalidInput` for malformed key/signature sizes, `ResourceLimit` for
/// excessive message size, or `AuthenticationFailed` for rejected signatures/weak points.
pub fn verify(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
    maximum_bytes: usize,
) -> Result<(), CryptoError> {
    tracing::trace!("Verifying an exact Ed25519 signature");
    if message.len() > maximum_bytes {
        return Err(CryptoError::ResourceLimit);
    }
    let bytes: &[u8; 32] = public_key
        .try_into()
        .map_err(|_| CryptoError::InvalidInput)?;
    let key = VerifyingKey::from_bytes(bytes).map_err(|_| CryptoError::InvalidInput)?;
    let signature = Signature::from_slice(signature).map_err(|_| CryptoError::InvalidInput)?;
    key.verify_strict(message, &signature)
        .map_err(|_| CryptoError::AuthenticationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::primitives::keys::signing_key;
    use crate::modules::cryptography::test_support::{ScriptedEntropy, transport};
    use std::collections::VecDeque;

    // CR-02 reconstructs exact role-bound AUTH bytes and compares literal upstream signatures.
    #[test]
    fn signatures_match_transport_auth_and_reject_changes() {
        let mut transcript = Vec::new();
        for role in ["initiator", "acceptor"] {
            let hello = transport(&format!("{role} HELLO"));
            transcript.extend_from_slice(&(hello.len() as u32).to_be_bytes());
            transcript.extend_from_slice(&hello);
        }
        for (role, role_byte) in [("initiator", 1u8), ("acceptor", 2u8)] {
            let mut entropy = ScriptedEntropy(VecDeque::from([Ok(transport(&format!(
                "{role} Ed25519 seed"
            )))]));
            let key = signing_key(&mut entropy).unwrap();
            let mut message = b"CR2SE-NETWORK-AUTH-V1\0".to_vec();
            message.push(role_byte);
            message.extend_from_slice(&transcript);
            let signature = sign(&key.private_key, &message, message.len()).unwrap();
            assert_eq!(
                signature.as_slice(),
                transport(&format!("{role} AUTH payload"))
            );
            verify(&key.public_key, &message, &signature, message.len()).unwrap();
            message[0] ^= 1;
            assert_eq!(
                verify(&key.public_key, &message, &signature, message.len()),
                Err(CryptoError::AuthenticationFailed)
            );
            message[0] ^= 1;
            let mut bad = signature;
            bad[5] ^= 1;
            assert!(verify(&key.public_key, &message, &bad, message.len()).is_err());
            assert!(verify(&key.public_key[..31], &message, &signature, message.len()).is_err());
            assert!(verify(&key.public_key, &message, &signature[..63], message.len()).is_err());
            assert_eq!(
                sign(&key.private_key, &message, 0),
                Err(CryptoError::ResourceLimit)
            );
            assert_eq!(
                verify(&key.public_key, &message, &signature, 0),
                Err(CryptoError::ResourceLimit)
            );
            let other = transport(if role == "initiator" {
                "acceptor Ed25519 public key"
            } else {
                "initiator Ed25519 public key"
            });
            assert!(verify(&other, &message, &signature, message.len()).is_err());
        }
    }

    // Exact identity-point bytes demonstrate the upstream weak-key acceptance ambiguity.
    #[test]
    fn weak_identity_point_signature_demonstrates_verification_ambiguity() {
        use ed25519_dalek::Verifier;
        let mut public = [0; 32];
        public[0] = 1;
        let mut bytes = [0; 64];
        bytes[0] = 1;
        let key = VerifyingKey::from_bytes(&public).unwrap();
        let signature = Signature::from_bytes(&bytes);
        for message in [b"first".as_slice(), b"different"] {
            assert!(key.verify(message, &signature).is_ok());
            assert_eq!(
                verify(&public, message, &bytes, 100),
                Err(CryptoError::AuthenticationFailed)
            );
        }
    }
}
