//! A placement: one provider's accepted agreement to retain one signed message.
//!
//! Callers: the store, the provider engine and recovery. A record holds everything the specification
//! lists (message metadata and signature, retention times, cumulative payments, state, receipt and removal
//! result); the payload bytes are kept beside it. A placement is never replaced.
use super::{
    ids::{PlacementId, RemovalId},
    metadata::{Metadata, SignedMessage},
    operation::Receipt,
    state::State,
};

/// The recorded outcome of a removal, kept for the lifetime of the ledger so a retry returns it identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Removal {
    /// The sender's replay identifier.
    pub removal_id: RemovalId,
    /// Credits issued back to the sender.
    pub release_credits: u64,
    /// The removal charge.
    pub price: u64,
}

/// Everything about a placement except the payload bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Provider-chosen placement identifier.
    pub id: PlacementId,
    /// Signed metadata of the stored message.
    pub metadata: Metadata,
    /// The sender's public key.
    pub sender_public_key: [u8; 32],
    /// The sender's signature over the message signing bytes.
    pub sender_signature: super::ids::Signature,
    /// Provider acceptance time.
    pub accepted_at: u64,
    /// Current expiry.
    pub expires_at: u64,
    /// Lifecycle state.
    pub state: State,
    /// Sum of the prices charged for the store and every renewal (the ceiling of any release credit).
    pub payments: u64,
    /// The price charged for the recovery, kept so an exact retry reports the original price; zero until recovered.
    pub recovery_price: u64,
    /// The recorded receipt once recovered.
    pub receipt: Option<Receipt>,
    /// The recorded removal once removed.
    pub removal: Option<Removal>,
}

/// A record together with its payload.
#[derive(Clone, PartialEq, Eq)]
pub struct Placement {
    /// The record.
    pub record: Record,
    /// The exact payload bytes.
    pub payload: Vec<u8>,
}

impl std::fmt::Debug for Placement {
    // The payload is application content and never printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Placement")
            .field("record", &self.record)
            .field("payload", &format_args!("{} bytes", self.payload.len()))
            .finish()
    }
}

impl Placement {
    /// The complete signed message this placement holds.
    pub fn message(&self) -> SignedMessage {
        tracing::trace!("Reassembling the signed message of a placement");
        SignedMessage {
            metadata: self.record.metadata.clone(),
            sender_public_key: self.record.sender_public_key,
            sender_signature: self.record.sender_signature,
            payload: self.payload.clone(),
        }
    }
}

impl Record {
    /// Whether the placement is active at `now`: pending or recovered and not yet expired.
    pub fn active_at(&self, now: u64) -> bool {
        self.state.is_active() && now < self.expires_at
    }
}
