//! Typed inputs and outputs of the hosted publication operations.
//!
//! Callers: the provider engine, the Services codec and requesters. These are the logical values of the specification, independent
//! of any wire encoding. The authenticated requester is supplied separately by Services and is never a field of a request, so a peer
//! cannot claim another identity.
use super::{
    ids::{ContentHash, PlacementId, PublicationId},
    metadata::{Kind, Publication, Visibility},
};
use crate::modules::identity::api::IdentityId;

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

/// A list request: one publisher's publications of one listed kind, newest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListRequest {
    /// Whose publications to list.
    pub publisher_id: IdentityId,
    /// `Page` or `FeedPost`; resources are never listed.
    pub kind: Kind,
    /// Exclusive cursor `(sequence, publicationId)` of the last entry already seen; both parts or neither.
    pub before: Option<(u64, PublicationId)>,
    /// Most entries wanted, in `1 ..= maximumListResults`.
    pub limit: u32,
}

/// Metadata of one listed publication; a hint that readers verify after fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListEntry {
    /// The placement holding it at this host.
    pub placement_id: PlacementId,
    /// The publication's identifier.
    pub publication_id: PublicationId,
    /// Page or feed post.
    pub kind: Kind,
    /// Publisher-assigned sequence.
    pub sequence: u64,
    /// The publisher's claimed time.
    pub published_at: u64,
    /// Public or restricted.
    pub visibility: Visibility,
    /// Content size in bytes.
    pub content_size: u64,
    /// SHA-256 of the content.
    pub content_hash: ContentHash,
    /// Host acceptance time.
    pub accepted_at: u64,
    /// Current expiry.
    pub expires_at: u64,
}

/// The result of a list, with the cursor of the next page derived from its last entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListOutput {
    /// The listed publisher.
    pub publisher_id: IdentityId,
    /// Entries, newest first by `(sequence, publicationId)` descending.
    pub entries: Vec<ListEntry>,
    /// Whether another readable entry followed the last returned one.
    pub more: bool,
}
