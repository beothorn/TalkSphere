//! Typed inputs and outputs of the six Messaging operations and the paid check.
//!
//! Callers: validation (step 7), the provider and requester engines. These are the logical values of
//! the specification: exact integer widths, no optional fields except the discovery cursor. Wire
//! encoding belongs to the Services and Peer Protocol layers.
use super::{
    ids::{PayloadHash, PlacementId, RecoveryId, RemovalId, Signature},
    metadata::SignedMessage,
};
use crate::modules::identity::api::IdentityId;

/// Retain one signed message for one recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreInput {
    /// The complete signed message, including its payload.
    pub message: SignedMessage,
    /// Requested initial retention in whole seconds.
    pub retention_seconds: u64,
}

/// Result of a successful store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreOutput {
    /// Provider-chosen placement identifier.
    pub placement_id: PlacementId,
    /// Provider acceptance time.
    pub accepted_at: u64,
    /// `accepted_at` plus the retention.
    pub expires_at: u64,
    /// Credits charged.
    pub price: u64,
}

/// The exclusive discovery cursor: both fields or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Acceptance time of the last returned entry.
    pub after_accepted_at: u64,
    /// Placement identifier of the last returned entry.
    pub after_placement_id: PlacementId,
}

/// List the authenticated recipient's pending placements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoverInput {
    /// Entries strictly after this pair, or the first page.
    pub cursor: Option<Cursor>,
    /// Page size, `1..=maximumDiscoverResults`.
    pub limit: u32,
}

/// Provider-visible description of one pending placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Placement identifier.
    pub placement_id: PlacementId,
    /// Message identifier shared by replicas.
    pub message_id: super::ids::MessageId,
    /// Sender identity as a hint; only recovery verification proves it.
    pub sender_id: IdentityId,
    /// Sender-claimed creation time.
    pub sender_created_at: u64,
    /// Payload encoding.
    pub encoding: super::state::Encoding,
    /// Payload length.
    pub payload_size: u64,
    /// Payload hash.
    pub payload_hash: PayloadHash,
    /// Provider acceptance time.
    pub accepted_at: u64,
    /// Placement expiry.
    pub expires_at: u64,
}

/// One page of discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverOutput {
    /// Entries ordered by `(accepted_at, placement_id)`.
    pub messages: Vec<Entry>,
    /// Whether another entry existed immediately after the last returned one.
    pub more: bool,
    /// Credits charged; an empty page is a charged success.
    pub price: u64,
}

/// Return one pending message to its authenticated recipient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoverInput {
    /// The placement to recover.
    pub placement_id: PlacementId,
    /// Expected message identifier.
    pub message_id: super::ids::MessageId,
    /// Expected payload hash.
    pub payload_hash: PayloadHash,
    /// Recipient-chosen replay identifier.
    pub recovery_id: RecoveryId,
    /// The recipient's Ed25519 public key.
    pub recipient_public_key: [u8; 32],
    /// The recipient's signature over the recovery request bytes.
    pub recipient_signature: Signature,
}

/// The recorded proof that a recovery happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Receipt {
    /// Recipient-chosen recovery identifier.
    pub recovery_id: RecoveryId,
    /// Provider time of the recovery commit.
    pub recovered_at: u64,
    /// The recipient's public key.
    pub recipient_public_key: [u8; 32],
    /// The recipient's recovery authorization signature.
    pub recipient_signature: Signature,
    /// The provider's signature over the receipt bytes.
    pub provider_signature: Signature,
}

/// Result of a successful recovery: the complete original message plus the receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverOutput {
    /// Placement identifier.
    pub placement_id: PlacementId,
    /// The original signed message.
    pub message: SignedMessage,
    /// The recorded receipt.
    pub receipt: Receipt,
}

/// Ask whether a placement was recovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusInput {
    /// The placement to ask about.
    pub placement_id: PlacementId,
}

/// The sender's view of one placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusOutput {
    /// Placement identifier.
    pub placement_id: PlacementId,
    /// Message identifier.
    pub message_id: super::ids::MessageId,
    /// Sender identity.
    pub sender_id: IdentityId,
    /// Recipient identity.
    pub recipient_id: IdentityId,
    /// Payload hash.
    pub payload_hash: PayloadHash,
    /// `pending` or `recovered`.
    pub state: super::state::State,
    /// Current expiry.
    pub expires_at: u64,
    /// The recorded receipt when recovered.
    pub receipt: Option<Receipt>,
    /// Credits charged.
    pub price: u64,
}

/// Extend retention of a pending or recovered placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenewInput {
    /// The placement to extend.
    pub placement_id: PlacementId,
    /// The expiry the sender believes is current.
    pub expected_expires_at: u64,
    /// Seconds to add.
    pub additional_seconds: u64,
}

/// Result of a renewal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenewOutput {
    /// Placement identifier.
    pub placement_id: PlacementId,
    /// Expiry before the renewal.
    pub previous_expires_at: u64,
    /// Expiry after the renewal.
    pub expires_at: u64,
    /// Credits charged.
    pub price: u64,
}

/// End a pending placement early for early-release credits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveInput {
    /// The placement to remove.
    pub placement_id: PlacementId,
    /// Sender-chosen replay identifier.
    pub removal_id: RemovalId,
    /// The expiry the sender believes is current.
    pub expected_expires_at: u64,
    /// The smallest release amount the sender accepts.
    pub minimum_release_credits: u64,
}

/// Result of a removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveOutput {
    /// Placement identifier.
    pub placement_id: PlacementId,
    /// Removal replay identifier.
    pub removal_id: RemovalId,
    /// Always true on success.
    pub removed: bool,
    /// Credits issued back to the sender.
    pub release_credits: u64,
    /// The removal charge.
    pub price: u64,
}

/// Challenge a pending placement for one payload byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckInput {
    /// The placement to challenge.
    pub placement_id: PlacementId,
    /// Zero-based payload offset.
    pub offset: u64,
}

/// One byte of evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckOutput {
    /// The challenged placement.
    pub placement_id: PlacementId,
    /// The challenged offset.
    pub offset: u64,
    /// The provider's byte at that offset.
    pub value: u8,
}
