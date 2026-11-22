//! The provider-side placement sizes used to price recoveries, renewals and checks.
//!
//! Callers: node composition of a provider, which installs it as the [`Placements`] of the provider's own
//! [`Client`](super::client::Client) (the client re-prices every quote on both sides). Dependencies: the placement store. A
//! provider knows a placement's size from its own durable record, and discloses it only to the placement's sender or recipient.
use super::client::Placements;
use crate::modules::{
    identity::api::IdentityId,
    messaging::{ids::PlacementId, store::Store},
    services::{Error, contract::OperationKey},
};
use std::sync::Arc;

/// Placement sizes read from the provider's own store.
pub struct StorePlacements(pub Arc<Store>);

impl Placements for StorePlacements {
    /// The stored payload size when the requester is the placement's sender or recipient; `Denied` for anyone else or an unknown id.
    fn payload_size(
        &self,
        key: OperationKey,
        _provider: IdentityId,
        placement: PlacementId,
    ) -> Result<u64, Error> {
        tracing::trace!("Reading a placement size from the provider's store");
        let record = self.0.record(placement).ok_or(Error::Denied)?;
        let metadata = &record.metadata;
        if key.requester != metadata.sender_id && key.requester != metadata.recipient_id {
            return Err(Error::Denied);
        }
        Ok(metadata.payload_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::messaging::{
        ids::{MessageId, PayloadHash, Signature},
        metadata::Metadata,
        placement::{Placement, Record},
        state::{Encoding, State},
        store::Limits,
    };

    // Scenario: a stored placement's size is shown to its sender and recipient and to nobody else; an unknown id is denied.
    #[test]
    fn only_the_sender_and_the_recipient_learn_a_stored_size() {
        let store = Arc::new(Store::volatile(Limits {
            placements: 4,
            payload_bytes: 1000,
        }));
        let (sender, recipient, other) = (
            IdentityId([1; 32]),
            IdentityId([3; 32]),
            IdentityId([5; 32]),
        );
        let id = PlacementId([9; 32]);
        store
            .insert(&Placement {
                record: Record {
                    id,
                    metadata: Metadata {
                        message_id: MessageId([1; 32]),
                        sender_id: sender,
                        recipient_id: recipient,
                        sender_created_at: 5,
                        encoding: Encoding::Opaque,
                        payload_size: 3,
                        payload_hash: PayloadHash([2; 32]),
                    },
                    sender_public_key: [7; 32],
                    sender_signature: Signature([8; 64]),
                    accepted_at: 10,
                    expires_at: 20,
                    state: State::Pending,
                    receipt: None,
                    payments: 0,
                    recovery_price: 0,
                    removal: None,
                },
                payload: b"abc".to_vec(),
            })
            .unwrap();
        let key = |who: IdentityId| OperationKey {
            requester: who,
            connection: [0; 32],
            stream: 5,
        };
        let placements = StorePlacements(store);
        assert_eq!(placements.payload_size(key(sender), recipient, id), Ok(3));
        assert_eq!(
            placements.payload_size(key(recipient), recipient, id),
            Ok(3)
        );
        assert_eq!(
            placements.payload_size(key(other), recipient, id),
            Err(Error::Denied)
        );
        assert_eq!(
            placements.payload_size(key(sender), recipient, PlacementId([8; 32])),
            Err(Error::Denied)
        );
    }
}
