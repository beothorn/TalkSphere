//! A placement: one host's accepted agreement to retain one signed publication.
//!
//! Callers: the store, the lifecycle rules and the provider operations. A record holds everything the specification lists (the
//! signed publication metadata and signature, retention times, cumulative payments, state and removal result); the content bytes are
//! kept beside it. A placement is never replaced. Unlike Messaging there is no recipient, no recovery transition and no receipt:
//! reading does not consume a publication.
use super::{
    ids::{PlacementId, RemovalId, Signature},
    metadata::{Metadata, Publication},
};

/// Lifecycle state of a placement: `pending -> removed | expired`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Durably accepted; readable while unexpired.
    Pending,
    /// The publisher ended it early.
    Removed,
    /// The retention ended while pending.
    Expired,
}

impl State {
    /// Whether moving to `next` is an allowed transition; terminal states never move.
    pub fn may_become(self, next: Self) -> bool {
        matches!((self, next), (Self::Pending, Self::Removed | Self::Expired))
    }

    /// Whether the host still owes retention in this state.
    pub fn is_active(self) -> bool {
        self == Self::Pending
    }

    /// The stored name of the state.
    pub fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Removed => "removed",
            Self::Expired => "expired",
        }
    }

    /// Decode a stored name exactly.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "pending" => Some(Self::Pending),
            "removed" => Some(Self::Removed),
            "expired" => Some(Self::Expired),
            _ => None,
        }
    }
}

/// The recorded outcome of a removal, kept for the lifetime of the ledger so a retry returns it identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Removal {
    /// The publisher's replay identifier.
    pub removal_id: RemovalId,
    /// Credits issued back to the publisher.
    pub release_credits: u64,
    /// The removal charge.
    pub price: u64,
}

/// Everything about a placement except the content bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Host-chosen placement identifier.
    pub id: PlacementId,
    /// Signed metadata of the stored publication.
    pub metadata: Metadata,
    /// The publisher's public key.
    pub publisher_public_key: [u8; 32],
    /// The publisher's signature over the publication signing bytes.
    pub publisher_signature: Signature,
    /// Host acceptance time.
    pub accepted_at: u64,
    /// Current expiry.
    pub expires_at: u64,
    /// Lifecycle state.
    pub state: State,
    /// Sum of the prices charged for the store and every renewal (the ceiling of any release credit).
    pub payments: u64,
    /// The recorded removal once removed.
    pub removal: Option<Removal>,
}

impl Record {
    /// Whether the placement is active at `now`: pending and not yet expired.
    pub fn active_at(&self, now: u64) -> bool {
        self.state.is_active() && now < self.expires_at
    }
}

/// A record together with its content.
#[derive(Clone, PartialEq, Eq)]
pub struct Placement {
    /// The record.
    pub record: Record,
    /// The exact content bytes (ciphertext when restricted).
    pub content: Vec<u8>,
}

impl std::fmt::Debug for Placement {
    // The content is application data and never printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Placement")
            .field("record", &self.record)
            .field("content", &format_args!("{} bytes", self.content.len()))
            .finish()
    }
}

impl Placement {
    /// The complete signed publication this placement holds, exactly as stored.
    pub fn publication(&self) -> Publication {
        tracing::trace!("Reassembling the signed publication of a placement");
        Publication {
            metadata: self.record.metadata.clone(),
            publisher_public_key: self.record.publisher_public_key,
            publisher_signature: self.record.publisher_signature,
            content: self.content.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Only pending moves, to removed or expired; terminal states are final and only pending is active.
    #[test]
    fn transitions_and_activity_follow_the_state_machine() {
        assert!(State::Pending.may_become(State::Removed));
        assert!(State::Pending.may_become(State::Expired));
        for from in [State::Removed, State::Expired] {
            for to in [State::Pending, State::Removed, State::Expired] {
                assert!(!from.may_become(to));
            }
            assert!(!from.is_active());
        }
        assert!(!State::Pending.may_become(State::Pending));
        assert!(State::Pending.is_active());
        for state in [State::Pending, State::Removed, State::Expired] {
            assert_eq!(State::parse(state.name()), Some(state));
        }
        assert_eq!(State::parse("recovered"), None);
    }
}
