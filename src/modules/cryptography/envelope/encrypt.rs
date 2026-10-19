//! Bounded encryption for a validated recipient with fresh per-object material.

use super::super::{
    api::EncryptRequest,
    credential,
    error::CryptoError,
    ports::Entropy,
    primitives::{aead, agreement, keys},
    random::random_bytes,
};
use super::{derivation, header::Header, object};

/// Encrypt a complete generic envelope for another identity or for the caller itself.
/// The recipient credential is validated before entropy consumption; there is no external AAD.
/// # Errors
/// Rejects invalid credentials, limits, unavailable entropy, invalid agreement or
/// encryption/allocation failure with no partial object and scoped secret cleanup.
pub fn encrypt(
    request: EncryptRequest<'_>,
    entropy: &mut impl Entropy,
) -> Result<Vec<u8>, CryptoError> {
    tracing::trace!("Encrypting a bounded object for a validated recipient");
    let recipient = credential::validate(request.recipient)?;
    let plaintext_length =
        u64::try_from(request.plaintext.len()).map_err(|_| CryptoError::ResourceLimit)?;
    let (sealed_length, total) = object::lengths_for_plaintext(plaintext_length, request.limits)?;
    let ephemeral = keys::ephemeral_key(entropy)?;
    let nonce = random_bytes::<24>(entropy)?;
    let header = Header {
        recipient_id: recipient.identity_id(),
        recipient_key: recipient.encryption_public_key(),
        ephemeral_key: ephemeral.public_key,
        nonce: *nonce,
        sealed_length,
    };
    let shared = agreement::ephemeral(ephemeral.private_key, &header.recipient_key)?;
    let key = derivation::derive(shared, &derivation::context(&header)?)?;
    let encoded_header = header.encode()?;
    let sealed = aead::seal(
        &key,
        &header.nonce,
        request.plaintext,
        &encoded_header,
        request.limits.maximum_object_bytes(),
    )?;
    let mut object = Vec::new();
    object
        .try_reserve_exact(total)
        .map_err(|_| CryptoError::ResourceLimit)?;
    object.extend_from_slice(&encoded_header);
    object.extend_from_slice(&sealed);
    Ok(object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::{
        config::ResourceLimits,
        test_support::{envelope, object_entropy, recipient},
    };
    // CR-04 compares the complete encryption result against independently pinned bytes.
    #[test]
    fn complete_encryption_matches_fixed_object() {
        let recipient = recipient();
        let plaintext = envelope("plaintext ASCII(\"CR2SE identity encryption test\")");
        let request = EncryptRequest {
            plaintext: &plaintext,
            recipient: &recipient,
            limits: ResourceLimits::new(30, 178).unwrap(),
        };
        let object = encrypt(request, &mut object_entropy()).unwrap();
        let mut expected = envelope("132-byte header");
        expected.extend(envelope("sealed ciphertext plus tag (46 bytes)"));
        assert_eq!(object, expected);
    }
    // Empty/UTF-8 objects retain exact lengths; invalid credentials and limits consume no entropy.
    #[test]
    fn encryption_boundaries_and_dependency_failure() {
        let mut recipient = recipient();
        let limits = ResourceLimits::new(1024, 1172).unwrap();
        for plaintext in [b"".as_slice(), "hello €".as_bytes()] {
            let bytes = encrypt(
                EncryptRequest {
                    plaintext,
                    recipient: &recipient,
                    limits,
                },
                &mut object_entropy(),
            )
            .unwrap();
            assert_eq!(bytes.len(), 148 + plaintext.len());
            assert_eq!(
                object::parse(&bytes, limits).unwrap().header.sealed_length,
                16 + plaintext.len() as u64
            );
        }
        let mut entropy = object_entropy();
        recipient.identity_id[0] ^= 1;
        assert!(
            encrypt(
                EncryptRequest {
                    plaintext: b"",
                    recipient: &recipient,
                    limits
                },
                &mut entropy
            )
            .is_err()
        );
        assert_eq!(entropy.0.len(), 2);
        recipient.identity_id[0] ^= 1;
        assert!(
            encrypt(
                EncryptRequest {
                    plaintext: b"ab",
                    recipient: &recipient,
                    limits: ResourceLimits::new(1, 148).unwrap()
                },
                &mut entropy
            )
            .is_err()
        );
        assert_eq!(entropy.0.len(), 2);
        entropy.0[1] = Err(CryptoError::Unavailable);
        assert_eq!(
            encrypt(
                EncryptRequest {
                    plaintext: b"",
                    recipient: &recipient,
                    limits
                },
                &mut entropy
            ),
            Err(CryptoError::Unavailable)
        );
    }
}
