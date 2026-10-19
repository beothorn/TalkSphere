//! Authenticate complete generic objects for the intended local persistent identity.

use super::super::{
    api::DecryptRequest,
    error::CryptoError,
    primitives::{aead, agreement},
    secrets::{Persistent, Secret},
};
use super::{derivation, object};

/// Decrypt for an authorized local identity after complete structure and recipient checks.
/// Private authority remains borrowed; successful plaintext belongs to the caller.
/// # Errors
/// Returns format/resource errors for invalid objects, authentication failure for
/// mismatched recipients or failed tags, and no provisional plaintext on any failure.
pub fn decrypt(request: DecryptRequest<'_, Secret<Persistent>>) -> Result<Vec<u8>, CryptoError> {
    tracing::trace!("Decrypting a complete object for the intended local identity");
    let parsed = object::parse(request.object, request.limits)?;
    if parsed.header.recipient_id != request.identity_id
        || parsed.header.recipient_key != request.encryption_public_key
    {
        return Err(CryptoError::AuthenticationFailed);
    }
    let shared = agreement::persistent(request.private_key, &parsed.header.ephemeral_key)?;
    let key = derivation::derive(shared, &derivation::context(&parsed.header)?)?;
    aead::open(
        &key,
        &parsed.header.nonce,
        parsed.sealed,
        parsed.encoded_header,
        request.limits.maximum_object_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::super::encrypt::encrypt;
    use super::*;
    use crate::modules::cryptography::{
        api::{EncryptRequest, RecipientCredential},
        config::ResourceLimits,
        credential,
        primitives::keys,
        test_support::{ScriptedEntropy, envelope, object_entropy, recipient},
    };
    use std::collections::VecDeque;
    use zeroize::Zeroizing;

    // A bounded local caller owns persistent authority and receives either complete plaintext or error.
    fn open(
        bytes: &[u8],
        recipient: &RecipientCredential,
        key: &Secret<Persistent>,
    ) -> Result<Vec<u8>, CryptoError> {
        decrypt(DecryptRequest {
            object: bytes,
            identity_id: recipient.identity_id,
            encryption_public_key: recipient.encryption_public_key,
            private_key: key,
            limits: ResourceLimits::new(1024, 1172).unwrap(),
        })
    }
    // CR-04/06 checks pinned decryption, every single-byte mutation and wrong local authority.
    #[test]
    fn pinned_decryption_rejects_every_mutation_without_plaintext() {
        let recipient = recipient();
        let key = Secret::new(Zeroizing::new(
            envelope("recipient raw persistent X25519 private-key input")
                .try_into()
                .unwrap(),
        ));
        let mut bytes = envelope("132-byte header");
        bytes.extend(envelope("sealed ciphertext plus tag (46 bytes)"));
        assert_eq!(
            open(&bytes, &recipient, &key).unwrap(),
            envelope("plaintext ASCII(\"CR2SE identity encryption test\")")
        );
        for index in 0..bytes.len() {
            let mut changed = bytes.clone();
            changed[index] ^= 1;
            assert!(open(&changed, &recipient, &key).is_err());
        }
        let wrong = Secret::new(Zeroizing::new([7; 32]));
        assert!(open(&bytes, &recipient, &wrong).is_err());
        let mut other = recipient;
        other.identity_id[0] ^= 1;
        assert!(open(&bytes, &other, &key).is_err());
        other = recipient;
        other.encryption_public_key[0] ^= 1;
        assert!(open(&bytes, &other, &key).is_err());
        assert!(open(&bytes[..bytes.len() - 1], &recipient, &key).is_err());
        bytes[68..100].fill(0);
        assert!(open(&bytes, &recipient, &key).is_err());
    }
    // CR-05 uses both the known local credential and a distinct generated recipient, including empty data.
    #[test]
    fn self_and_other_recipient_round_trips() {
        let mut entropy = ScriptedEntropy(VecDeque::from([Ok(vec![11; 32]), Ok(vec![12; 32])]));
        let signing = keys::signing_key(&mut entropy).unwrap();
        let encryption = keys::persistent_key(&mut entropy).unwrap();
        let other = RecipientCredential {
            identity_id: credential::identity_id(&signing.public_key).unwrap(),
            signing_public_key: signing.public_key,
            encryption_public_key: encryption.public_key,
            binding_signature: credential::bind(&signing.private_key, &encryption.public_key)
                .unwrap(),
        };
        let local_key = Secret::new(Zeroizing::new(
            envelope("recipient raw persistent X25519 private-key input")
                .try_into()
                .unwrap(),
        ));
        for (recipient, key) in [(recipient(), &local_key), (other, &encryption.private_key)] {
            for plaintext in [b"".as_slice(), "hello €".as_bytes()] {
                let bytes = encrypt(
                    EncryptRequest {
                        plaintext,
                        recipient: &recipient,
                        limits: ResourceLimits::new(1024, 1172).unwrap(),
                    },
                    &mut object_entropy(),
                )
                .unwrap();
                assert_eq!(open(&bytes, &recipient, key).unwrap(), plaintext);
            }
        }
    }
}
