//! The exact message signing bytes, sender signing and provider-side verification.
//!
//! Callers: a sender creating a message, a provider validating a store and a recipient validating a
//! recovered message. The signature covers the exact stored representation: when the payload is
//! encrypted it covers the ciphertext, never the plaintext. Unsigned integers are eight-byte big-endian,
//! identifiers and hashes are their exact 32 bytes, and the domain string is ASCII without a terminator.
use super::{
    error::Error,
    ids::{MessageId, PayloadHash},
    metadata::{Metadata, SignedMessage},
    ports::Signer,
    state::Encoding,
};
use crate::modules::{
    cryptography::{
        credential,
        primitives::{hash, signature},
    },
    identity::api::IdentityId,
};

/// Domain string that starts every message signing input.
pub const DOMAIN: &[u8] = b"CR2SE-MESSAGE-V1";
/// Largest signing input handled, covering the finite profile's payload bound with room to spare.
pub const MAXIMUM_SIGNING_BYTES: usize = 1 << 17;

/// The message signing bytes: `DOMAIN || messageId || senderId || recipientId || u64be(createdAt) ||
/// encodingCode || u64be(size) || payloadHash || payload`.
pub fn signing_bytes(metadata: &Metadata, payload: &[u8]) -> Vec<u8> {
    tracing::trace!("Building the message signing bytes");
    let mut bytes = Vec::with_capacity(DOMAIN.len() + 32 * 4 + 17 + payload.len());
    bytes.extend_from_slice(DOMAIN);
    bytes.extend_from_slice(&metadata.message_id.0);
    bytes.extend_from_slice(&metadata.sender_id.0);
    bytes.extend_from_slice(&metadata.recipient_id.0);
    bytes.extend_from_slice(&metadata.sender_created_at.to_be_bytes());
    bytes.push(metadata.encoding.code());
    bytes.extend_from_slice(&metadata.payload_size.to_be_bytes());
    bytes.extend_from_slice(&metadata.payload_hash.0);
    bytes.extend_from_slice(payload);
    bytes
}

/// Create a signed message from a payload, as a sender does before storing it.
/// # Errors
/// `Invalid` for an empty or oversized payload; `Unavailable` when the signer cannot sign.
pub fn sign_message(
    signer: &dyn Signer,
    recipient_id: IdentityId,
    message_id: MessageId,
    sender_created_at: u64,
    encoding: Encoding,
    payload: Vec<u8>,
) -> Result<SignedMessage, Error> {
    tracing::trace!("Signing a message for storage");
    if payload.is_empty() || payload.len() > MAXIMUM_SIGNING_BYTES - 1024 {
        return Err(Error::Invalid);
    }
    let payload_hash =
        PayloadHash(hash::sha256(&payload, payload.len()).map_err(|_| Error::Invalid)?);
    let metadata = Metadata {
        message_id,
        sender_id: signer.identity(),
        recipient_id,
        sender_created_at,
        encoding,
        payload_size: payload.len() as u64,
        payload_hash,
    };
    let sender_signature = signer.sign(&signing_bytes(&metadata, &payload))?;
    Ok(SignedMessage {
        metadata,
        sender_public_key: signer.public_key(),
        sender_signature,
        payload,
    })
}

/// Verify a signed message the way a provider does before accepting a store, in the specified order:
/// the sender ID derives from the public key; it equals the authenticated requester when one is given; the
/// payload size and SHA-256 match; the Ed25519 signature verifies over the exact signing bytes. A recipient
/// repeats the same checks after recovery with `authenticated_sender` of `None`.
/// # Errors
/// `Invalid` for the first check that fails; the message is never partly accepted.
pub fn verify_message(
    message: &SignedMessage,
    authenticated_sender: Option<IdentityId>,
) -> Result<(), Error> {
    tracing::trace!("Verifying a signed message");
    let m = &message.metadata;
    let derived =
        credential::identity_id(&message.sender_public_key).map_err(|_| Error::Invalid)?;
    if derived != m.sender_id.0 {
        return Err(Error::Invalid);
    }
    if authenticated_sender.is_some_and(|caller| caller != m.sender_id) {
        return Err(Error::Invalid);
    }
    if m.payload_size == 0 || message.payload.len() as u64 != m.payload_size {
        return Err(Error::Invalid);
    }
    let actual =
        hash::sha256(&message.payload, message.payload.len()).map_err(|_| Error::Invalid)?;
    if actual != m.payload_hash.0 {
        return Err(Error::Invalid);
    }
    let bytes = signing_bytes(m, &message.payload);
    signature::verify(
        &message.sender_public_key,
        &bytes,
        &message.sender_signature.0,
        MAXIMUM_SIGNING_BYTES,
    )
    .map_err(|_| Error::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::messaging::{
        fixture_tests::bytes as hex_bytes, test_support::TestIdentity,
    };
    use serde_json::Value;

    fn vectors() -> Value {
        serde_json::from_str(include_str!("fixtures/vectors.json")).unwrap()
    }

    // The literal MS-01 example: a message from A to C, payload `abc`, opaque, at time 1000.
    fn fixture_message() -> SignedMessage {
        let v = vectors();
        let m = &v["message"];
        let (sender, recipient) = (TestIdentity::new(1), TestIdentity::new(3));
        let mut message_id = [0u8; 32];
        message_id.copy_from_slice(&hex_bytes(&m["messageId"]));
        let signer: &dyn Signer = &sender;
        sign_message(
            signer,
            recipient.identity(),
            MessageId(message_id),
            1000,
            Encoding::Opaque,
            b"abc".to_vec(),
        )
        .unwrap()
    }

    // MS-01: the signing bytes equal the independently generated literal, the signature is the 64-byte Ed25519 value
    // the independent generator produced, and the message verifies.
    #[test]
    fn ms_01_signing_bytes_and_signature_match_the_independent_vector() {
        let v = vectors();
        let message = fixture_message();
        let expected = hex_bytes(&v["message"]["signingBytes"]);
        assert_eq!(signing_bytes(&message.metadata, &message.payload), expected);
        assert_eq!(&expected[..16], b"CR2SE-MESSAGE-V1");
        assert_eq!(
            message.sender_signature.0.to_vec(),
            hex_bytes(&v["message"]["senderSignature"])
        );
        assert_eq!(
            message.sender_public_key.to_vec(),
            hex_bytes(&v["identities"]["sender"]["publicKey"])
        );
        assert_eq!(
            verify_message(&message, Some(message.metadata.sender_id)),
            Ok(())
        );
        assert_eq!(verify_message(&message, None), Ok(()));
    }

    // MS-01: changing any metadata field, the payload, the key or the signature, one at a time while keeping the
    // original signature, is rejected.
    #[test]
    fn ms_01_every_mutation_is_rejected() {
        let good = fixture_message();
        let mutations: Vec<(&str, fn(&mut SignedMessage))> = vec![
            ("messageId", |m| m.metadata.message_id.0[0] ^= 1),
            ("senderId", |m| m.metadata.sender_id.0[0] ^= 1),
            ("recipientId", |m| m.metadata.recipient_id.0[0] ^= 1),
            ("createdAt", |m| m.metadata.sender_created_at += 1),
            ("encoding", |m| {
                m.metadata.encoding = Encoding::IdentityEncryptedV1
            }),
            ("size", |m| m.metadata.payload_size += 1),
            ("hash", |m| m.metadata.payload_hash.0[0] ^= 1),
            ("payload", |m| m.payload[0] ^= 1),
            ("key", |m| m.sender_public_key[0] ^= 1),
            ("signature", |m| m.sender_signature.0[63] ^= 1),
        ];
        for (name, mutate) in mutations {
            let mut message = good.clone();
            mutate(&mut message);
            assert_eq!(
                verify_message(&message, None),
                Err(Error::Invalid),
                "{name}"
            );
        }
    }

    // A correctly signed message whose sender is not the authenticated caller is refused, and the checks run in order:
    // a wrong sender ID is refused before the signature is consulted.
    #[test]
    fn the_sender_must_be_the_authenticated_caller() {
        let message = fixture_message();
        assert_eq!(
            verify_message(&message, Some(IdentityId([9; 32]))),
            Err(Error::Invalid)
        );
        // Re-signing with a different identity but claiming A's ID fails the derivation check.
        let other = TestIdentity::new(5);
        let mut forged = message.clone();
        forged.sender_public_key = other.key();
        assert_eq!(verify_message(&forged, None), Err(Error::Invalid));
    }

    // A payload that is empty, oversized or whose declared size is zero cannot be signed or verified.
    #[test]
    fn empty_and_oversized_payloads_are_refused() {
        let sender = TestIdentity::new(1);
        let to = IdentityId([3; 32]);
        assert_eq!(
            sign_message(&sender, to, MessageId([1; 32]), 1, Encoding::Opaque, vec![]).err(),
            Some(Error::Invalid)
        );
        assert_eq!(
            sign_message(
                &sender,
                to,
                MessageId([1; 32]),
                1,
                Encoding::Opaque,
                vec![0; MAXIMUM_SIGNING_BYTES]
            )
            .err(),
            Some(Error::Invalid)
        );
        let mut message = fixture_message();
        message.metadata.payload_size = 0;
        message.payload.clear();
        assert_eq!(verify_message(&message, None), Err(Error::Invalid));
    }

    // A signer that cannot sign makes signing fail without producing a message.
    #[test]
    fn a_broken_signer_fails_cleanly() {
        let sender = TestIdentity::new(1);
        sender.break_signing();
        assert_eq!(
            sign_message(
                &sender,
                IdentityId([3; 32]),
                MessageId([1; 32]),
                1,
                Encoding::Opaque,
                b"x".to_vec()
            )
            .err(),
            Some(Error::Unavailable)
        );
    }

    // For an encrypted payload the signature covers the ciphertext bytes exactly as stored, so changing one byte of
    // the stored representation is detected.
    #[test]
    fn the_signature_covers_the_stored_representation() {
        let sender = TestIdentity::new(1);
        let mut message = sign_message(
            &sender,
            IdentityId([3; 32]),
            MessageId([1; 32]),
            5,
            Encoding::IdentityEncryptedV1,
            vec![7; 105],
        )
        .unwrap();
        assert_eq!(verify_message(&message, None), Ok(()));
        message.payload[104] ^= 1;
        assert_eq!(verify_message(&message, None), Err(Error::Invalid));
    }
}
