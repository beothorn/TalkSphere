//! Typed inputs and outputs of the hosted publication operations.
//!
//! Callers: the provider engine, the Services codec and requesters. These are the logical values of the specification, independent
//! of any wire encoding. The authenticated requester is supplied separately by Services and is never a field of a request, so a peer
//! cannot claim another identity.
use super::{
    ids::{PlacementId, PublicationId},
    metadata::Publication,
};

/// A store request: one complete signed publication and the initial retention in seconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreRequest {
    /// The complete signed publication, exactly as the publisher created it.
    pub publication: Publication,
    /// Requested initial retention in whole seconds.
    pub retention_seconds: u64,
}

/// What an accepted store reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreOutput {
    /// The host-generated placement identifier.
    pub placement_id: PlacementId,
    /// The stored publication's identifier.
    pub publication_id: PublicationId,
    /// Host acceptance time.
    pub accepted_at: u64,
    /// Expiry of the placement: acceptance plus the retention.
    pub expires_at: u64,
}
