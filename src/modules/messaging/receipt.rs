//! Recovery request and provider receipt signatures.
//!
//! Callers: a recipient signs a recovery request, the provider verifies it and signs a receipt, and the
//! sender (through status) and the recipient verify both. Together the two signatures show that the
//! recipient authorized this specific recovery and that the provider recorded its completion; they do not
//! prove what anyone read. Layouts are the exact ones of the specification: identifiers and hashes are
//! their 32 bytes, the recovery time is eight-byte big-endian, domain strings are ASCII without terminator.
use super::{
    error::Error,
    ids::{MessageId, PayloadHash, PlacementId, RecoveryId, Signature},
    operation::{Receipt, RecoverInput},
    ports::Signer,
    signing::MAXIMUM_SIGNING_BYTES,
};
use crate::modules::{
    cryptography::{credential, primitives::signature},
    identity::api::IdentityId,
};

/// Domain string of a recovery request.
pub const REQUEST_DOMAIN: &[u8] = b"CR2SE-MESSAGE-RECOVERY-REQUEST-V1";
/// Domain string of a provider recovery receipt.
pub const RECEIPT_DOMAIN: &[u8] = b"CR2SE-MESSAGE-RECOVERY-RECEIPT-V1";

/// Everything a receipt binds besides the recovery itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// The provider that holds the placement.
    pub provider_id: IdentityId,
    /// The placement.
    pub placement_id: PlacementId,
    /// The message.
    pub message_id: MessageId,
    /// The sender.
    pub sender_id: IdentityId,
    /// The recipient.
    pub recipient_id: IdentityId,
    /// The payload hash.
    pub payload_hash: PayloadHash,
}

/// The recovery request signing bytes: `REQUEST_DOMAIN || providerId || placementId || messageId || payloadHash || recoveryId`.
pub fn request_bytes(
    provider_id: IdentityId,
    placement_id: PlacementId,
    message_id: MessageId,
    payload_hash: PayloadHash,
    recovery_id: RecoveryId,
) -> Vec<u8> {
    tracing::trace!("Building the recovery request signing bytes");
    let mut bytes = Vec::with_capacity(REQUEST_DOMAIN.len() + 32 * 5);
    bytes.extend_from_slice(REQUEST_DOMAIN);
    for part in [
        &provider_id.0,
        &placement_id.0,
        &message_id.0,
        &payload_hash.0,
        &recovery_id.0,
    ] {
        bytes.extend_from_slice(part);
    }
    bytes
}

/// The provider receipt signing bytes: `RECEIPT_DOMAIN || providerId || placementId || messageId || senderId || recipientId ||
/// payloadHash || recoveryId || u64be(recoveredAt)`.
pub fn receipt_bytes(binding: &Binding, recovery_id: RecoveryId, recovered_at: u64) -> Vec<u8> {
    tracing::trace!("Building the provider receipt signing bytes");
    let mut bytes = Vec::with_capacity(RECEIPT_DOMAIN.len() + 32 * 7 + 8);
    bytes.extend_from_slice(RECEIPT_DOMAIN);
    for part in [
        &binding.provider_id.0,
        &binding.placement_id.0,
        &binding.message_id.0,
        &binding.sender_id.0,
        &binding.recipient_id.0,
        &binding.payload_hash.0,
        &recovery_id.0,
    ] {
        bytes.extend_from_slice(part);
    }
    bytes.extend_from_slice(&recovered_at.to_be_bytes());
    bytes
}

/// Create a signed recovery request, as the recipient does.
/// # Errors
/// `Unavailable` when the signer cannot sign.
pub fn sign_request(
    recipient: &dyn Signer,
    provider_id: IdentityId,
    placement_id: PlacementId,
    message_id: MessageId,
    payload_hash: PayloadHash,
    recovery_id: RecoveryId,
) -> Result<RecoverInput, Error> {
    tracing::trace!("Signing a recovery request");
    let signature = recipient.sign(&request_bytes(
        provider_id,
        placement_id,
        message_id,
        payload_hash,
        recovery_id,
    ))?;
    Ok(RecoverInput {
        placement_id,
        message_id,
        payload_hash,
        recovery_id,
        recipient_public_key: recipient.public_key(),
        recipient_signature: signature,
    })
}

/// Verify a recovery request at a provider: the recipient ID derived from the key equals the placement's recipient, and the
/// signature covers this provider, placement, message, hash and recovery ID.
/// # Errors
/// `NotAvailable` when the key does not belong to the recipient (concealed like any unauthorized request); `Invalid` for a bad signature.
pub fn verify_request(
    input: &RecoverInput,
    provider_id: IdentityId,
    recipient_id: IdentityId,
) -> Result<(), Error> {
    tracing::trace!("Verifying a recovery request");
    let derived =
        credential::identity_id(&input.recipient_public_key).map_err(|_| Error::Invalid)?;
    if derived != recipient_id.0 {
        return Err(Error::NotAvailable);
    }
    let bytes = request_bytes(
        provider_id,
        input.placement_id,
        input.message_id,
        input.payload_hash,
        input.recovery_id,
    );
    verify(
        &input.recipient_public_key,
        &bytes,
        &input.recipient_signature,
    )
}

/// Sign the provider receipt for a completed recovery.
/// # Errors
/// `Unavailable` when the provider cannot sign.
pub fn sign_receipt(
    provider: &dyn Signer,
    binding: &Binding,
    recovery_id: RecoveryId,
    recovered_at: u64,
) -> Result<Signature, Error> {
    tracing::trace!("Signing a provider recovery receipt");
    provider.sign(&receipt_bytes(binding, recovery_id, recovered_at))
}

/// Verify both signatures of a recorded recovery, as the sender does with a recovered status and the recipient with its own receipt:
/// the recipient's authorization for this provider and placement, and the provider's receipt under its public key.
/// # Errors
/// `Invalid` when either signature, or the recipient key's identity, does not verify.
pub fn verify_receipt(
    receipt: &Receipt,
    binding: &Binding,
    provider_public_key: &[u8; 32],
) -> Result<(), Error> {
    tracing::trace!("Verifying both recovery signatures");
    let derived = credential::identity_id(provider_public_key).map_err(|_| Error::Invalid)?;
    let recipient =
        credential::identity_id(&receipt.recipient_public_key).map_err(|_| Error::Invalid)?;
    if derived != binding.provider_id.0 || recipient != binding.recipient_id.0 {
        return Err(Error::Invalid);
    }
    let request = request_bytes(
        binding.provider_id,
        binding.placement_id,
        binding.message_id,
        binding.payload_hash,
        receipt.recovery_id,
    );
    verify(
        &receipt.recipient_public_key,
        &request,
        &receipt.recipient_signature,
    )?;
    let bytes = receipt_bytes(binding, receipt.recovery_id, receipt.recovered_at);
    verify(provider_public_key, &bytes, &receipt.provider_signature)
}

// Ordinary Ed25519 verification over exact bytes.
fn verify(public_key: &[u8; 32], bytes: &[u8], signature_value: &Signature) -> Result<(), Error> {
    signature::verify(public_key, bytes, &signature_value.0, MAXIMUM_SIGNING_BYTES)
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
    fn array32(value: &Value) -> [u8; 32] {
        hex_bytes(value).try_into().unwrap()
    }
    // The literal MS-06 and MS-07 scenario: C recovers A's message at B.
    fn binding() -> (Binding, RecoveryId, u64) {
        let v = vectors();
        (
            Binding {
                provider_id: IdentityId(array32(&v["identities"]["provider"]["id"])),
                placement_id: PlacementId(array32(&v["recovery"]["placementId"])),
                message_id: MessageId(array32(&v["message"]["messageId"])),
                sender_id: IdentityId(array32(&v["identities"]["sender"]["id"])),
                recipient_id: IdentityId(array32(&v["identities"]["recipient"]["id"])),
                payload_hash: PayloadHash(array32(&v["message"]["payloadHash"])),
            },
            RecoveryId(array32(&v["recovery"]["recoveryId"])),
            v["recovery"]["recoveredAt"].as_u64().unwrap(),
        )
    }
    fn signed(b: &Binding, recovery: RecoveryId) -> RecoverInput {
        sign_request(
            &TestIdentity::new(3),
            b.provider_id,
            b.placement_id,
            b.message_id,
            b.payload_hash,
            recovery,
        )
        .unwrap()
    }

    // MS-06 and MS-07: the request and receipt bytes equal the independent literals, the signatures are the independent
    // ones byte for byte, and both verify.
    #[test]
    fn request_and_receipt_match_the_independent_vector() {
        let v = vectors();
        let (b, recovery, at) = binding();
        let expected_request = hex_bytes(&v["recovery"]["requestBytes"]);
        assert_eq!(
            request_bytes(
                b.provider_id,
                b.placement_id,
                b.message_id,
                b.payload_hash,
                recovery
            ),
            expected_request
        );
        let input = signed(&b, recovery);
        assert_eq!(
            input.recipient_signature.0.to_vec(),
            hex_bytes(&v["recovery"]["recipientSignature"])
        );
        assert_eq!(
            verify_request(&input, b.provider_id, b.recipient_id),
            Ok(())
        );
        let expected_receipt = hex_bytes(&v["recovery"]["receiptBytes"]);
        assert_eq!(receipt_bytes(&b, recovery, at), expected_receipt);
        let provider = TestIdentity::new(2);
        let provider_signature = sign_receipt(&provider, &b, recovery, at).unwrap();
        assert_eq!(
            provider_signature.0.to_vec(),
            hex_bytes(&v["recovery"]["providerSignature"])
        );
        let receipt = Receipt {
            recovery_id: recovery,
            recovered_at: at,
            recipient_public_key: input.recipient_public_key,
            recipient_signature: input.recipient_signature,
            provider_signature,
        };
        assert_eq!(verify_receipt(&receipt, &b, &provider.key()), Ok(()));
    }

    // MS-06: a request mutated in the provider, placement, message, hash, recovery ID, key or signature is not accepted.
    #[test]
    fn mutated_requests_are_rejected() {
        let (b, recovery, _) = binding();
        let good = signed(&b, recovery);
        let cases: [(&str, fn(&mut RecoverInput)); 5] = [
            ("placement", |i| i.placement_id.0[0] ^= 1),
            ("message", |i| i.message_id.0[0] ^= 1),
            ("hash", |i| i.payload_hash.0[0] ^= 1),
            ("recovery", |i| i.recovery_id.0[0] ^= 1),
            ("signature", |i| i.recipient_signature.0[5] ^= 1),
        ];
        for (name, mutate) in cases {
            let mut input = good.clone();
            mutate(&mut input);
            assert_eq!(
                verify_request(&input, b.provider_id, b.recipient_id),
                Err(Error::Invalid),
                "{name}"
            );
        }
        // Another provider's ID breaks the binding even for an otherwise valid request.
        assert_eq!(
            verify_request(&good, IdentityId([9; 32]), b.recipient_id),
            Err(Error::Invalid)
        );
        // A request signed by someone who is not the recipient is concealed like any unauthorized request.
        let intruder = sign_request(
            &TestIdentity::new(5),
            b.provider_id,
            b.placement_id,
            b.message_id,
            b.payload_hash,
            recovery,
        )
        .unwrap();
        assert_eq!(
            verify_request(&intruder, b.provider_id, b.recipient_id),
            Err(Error::NotAvailable)
        );
        let mut swapped = good.clone();
        swapped.recipient_public_key = TestIdentity::new(5).key();
        assert_eq!(
            verify_request(&swapped, b.provider_id, b.recipient_id),
            Err(Error::NotAvailable)
        );
    }

    // MS-07: a receipt changed in any bound value, time, either signature or either key fails verification.
    #[test]
    fn mutated_receipts_are_rejected() {
        let (b, recovery, at) = binding();
        let input = signed(&b, recovery);
        let provider = TestIdentity::new(2);
        let good = Receipt {
            recovery_id: recovery,
            recovered_at: at,
            recipient_public_key: input.recipient_public_key,
            recipient_signature: input.recipient_signature,
            provider_signature: sign_receipt(&provider, &b, recovery, at).unwrap(),
        };
        assert_eq!(verify_receipt(&good, &b, &provider.key()), Ok(()));
        let receipt_cases: [(&str, fn(&mut Receipt)); 4] = [
            ("time", |r| r.recovered_at += 1),
            ("recovery", |r| r.recovery_id.0[0] ^= 1),
            ("provider signature", |r| r.provider_signature.0[0] ^= 1),
            ("recipient signature", |r| r.recipient_signature.0[0] ^= 1),
        ];
        for (name, mutate) in receipt_cases {
            let mut receipt = good.clone();
            mutate(&mut receipt);
            assert_eq!(
                verify_receipt(&receipt, &b, &provider.key()),
                Err(Error::Invalid),
                "{name}"
            );
        }
        let binding_cases: [(&str, fn(&mut Binding)); 4] = [
            ("sender", |b| b.sender_id.0[0] ^= 1),
            ("placement", |b| b.placement_id.0[0] ^= 1),
            ("hash", |b| b.payload_hash.0[0] ^= 1),
            ("recipient", |b| b.recipient_id.0[0] ^= 1),
        ];
        for (name, mutate) in binding_cases {
            let mut other = b;
            mutate(&mut other);
            assert_eq!(
                verify_receipt(&good, &other, &provider.key()),
                Err(Error::Invalid),
                "{name}"
            );
        }
        // A different key cannot stand in for the provider.
        assert_eq!(
            verify_receipt(&good, &b, &TestIdentity::new(5).key()),
            Err(Error::Invalid)
        );
    }

    // A recipient or provider that cannot sign makes signing fail without producing a value.
    #[test]
    fn broken_signers_fail_cleanly() {
        let (b, recovery, at) = binding();
        let recipient = TestIdentity::new(3);
        recipient.break_signing();
        assert_eq!(
            sign_request(
                &recipient,
                b.provider_id,
                b.placement_id,
                b.message_id,
                b.payload_hash,
                recovery
            )
            .err(),
            Some(Error::Unavailable)
        );
        let provider = TestIdentity::new(2);
        provider.break_signing();
        assert_eq!(
            sign_receipt(&provider, &b, recovery, at).err(),
            Some(Error::Unavailable)
        );
    }
}
