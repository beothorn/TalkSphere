//! Provider-side placement sizes for pricing.
//!
//! Callers: node composition, which installs it on a host's own `Client`. The client re-prices every quote on both sides, and a renewal's price depends on the
//! content size of the placement. A requester states what it knows from its own catalog; a host reads the size from its own durable placements, never from the
//! requester, and discloses it only to the placement's publisher.
use super::client::Placements;
use crate::modules::{
    hosted_publications::{ids::PlacementId, store::Store},
    identity::api::IdentityId,
    services::{Error, contract::OperationKey},
};
use std::sync::Arc;

/// The `Placements` port backed by a host's own placement store.
pub struct StorePlacements(pub Arc<Store>);

impl Placements for StorePlacements {
    // The content size recorded in the host's store, for the placement's own publisher only: any other requester (or an unknown placement) is a concealed denial.
    fn content_size(
        &self,
        key: OperationKey,
        _provider: IdentityId,
        placement: PlacementId,
    ) -> Result<u64, Error> {
        tracing::trace!("Reading a hosted placement size from the host's own store");
        self.0
            .record(placement)
            .filter(|record| record.metadata.publisher_id == key.requester)
            .map(|record| record.metadata.content_size)
            .ok_or(Error::Denied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::hosted_publications::store::tests::{LIMITS, placement};
    use crate::modules::hosted_publications::{metadata::Kind, store::Store};

    // A host reports a placement's size to its publisher only; another requester and an unknown placement are denied.
    #[test]
    fn sizes_are_disclosed_to_the_publisher_only() {
        let store = Arc::new(Store::volatile(LIMITS));
        store
            .insert(&placement(1, 3, Kind::Page, 1, 1000, 3), 1000)
            .unwrap();
        let ports = StorePlacements(store);
        let key = |who: u8| OperationKey {
            requester: IdentityId([who; 32]),
            connection: [1; 32],
            stream: 1,
        };
        let host = IdentityId([9; 32]);
        assert_eq!(
            ports.content_size(key(3), host, PlacementId([1; 32])),
            Ok(3)
        );
        assert_eq!(
            ports.content_size(key(4), host, PlacementId([1; 32])),
            Err(Error::Denied)
        );
        assert_eq!(
            ports.content_size(key(3), host, PlacementId([2; 32])),
            Err(Error::Denied)
        );
    }
}
