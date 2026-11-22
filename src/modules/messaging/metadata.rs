//! Message metadata and the complete signed message value.
//!
//! Callers: signing, store validation, placement persistence and recovery. Metadata is visible to every
//! provider that holds a placement; encryption protects only the payload.
use super::{
    ids::{MessageId, PayloadHash, Signature},
    state::Encoding,
};
use crate::modules::identity::api::IdentityId;

/// The signed, provider-visible facts about one message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    /// Sender-chosen message identifier.
    pub message_id: MessageId,
    /// Sender identity.
    pub sender_id: IdentityId,
    /// The one identity allowed to discover and recover the message.
    pub recipient_id: IdentityId,
    /// Sender-claimed creation time in Unix seconds; not a trusted clock reading.
    pub sender_created_at: u64,
    /// How to read the payload.
    pub encoding: Encoding,
    /// Exact payload length in bytes, at least one.
    pub payload_size: u64,
    /// SHA-256 of the payload.
    pub payload_hash: PayloadHash,
}

/// A message with its proof of origin and payload.
#[derive(Clone, PartialEq, Eq)]
pub struct SignedMessage {
    /// Provider-visible metadata.
    pub metadata: Metadata,
    /// The sender's 32-byte Ed25519 public key.
    pub sender_public_key: [u8; 32],
    /// The sender's signature over the message signing bytes.
    pub sender_signature: Signature,
    /// The exact payload bytes (plaintext, ciphertext or an application format).
    pub payload: Vec<u8>,
}

impl std::fmt::Debug for SignedMessage {
    // The payload is application content and is never printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedMessage")
            .field("metadata", &self.metadata)
            .field("payload", &format_args!("{} bytes", self.payload.len()))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A signed message's debug form shows only the payload length, never its bytes.
    #[test]
    fn debug_never_prints_the_payload() {
        let message = SignedMessage {
            metadata: Metadata {
                message_id: MessageId([1; 32]),
                sender_id: IdentityId([2; 32]),
                recipient_id: IdentityId([3; 32]),
                sender_created_at: 1000,
                encoding: Encoding::Opaque,
                payload_size: 3,
                payload_hash: PayloadHash([4; 32]),
            },
            sender_public_key: [5; 32],
            sender_signature: Signature([6; 64]),
            payload: b"abc".to_vec(),
        };
        let text = format!("{message:?}");
        assert!(text.contains("3 bytes") && !text.contains("97"), "{text}");
    }
}
