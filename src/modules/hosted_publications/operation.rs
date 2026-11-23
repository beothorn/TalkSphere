//! Typed inputs and outputs of the hosted publication operations.
//!
//! Callers: the provider engine, the Services codec and requesters. These are the logical values of the specification, independent
//! of any wire encoding. The authenticated requester is supplied separately by Services and is never a field of a request, so a peer
//! cannot claim another identity.
use super::{
    ids::{ContentHash, PlacementId, PublicationId, RemovalId},
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

/// A get request: one complete publication by its publisher and identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetRequest {
    /// The publisher the reader intended; a different publisher's publication is never returned.
    pub publisher_id: IdentityId,
    /// The wanted publication.
    pub publication_id: PublicationId,
    /// The largest content the reader accepts, in `1 ..= maximumContentBytes`.
    pub maximum_bytes: u64,
}

/// The complete stored publication, exactly the signed bytes, for the reader to verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetOutput {
    /// The publication as stored.
    pub publication: Publication,
}

/// A renewal request for one pending placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenewRequest {
    /// The placement to extend.
    pub placement_id: PlacementId,
    /// The expiry the publisher believes is current; renewal applies only to exactly this value.
    pub expected_expires_at: u64,
    /// Seconds to add, within the offering's retention limits.
    pub additional_seconds: u64,
}

/// What an accepted renewal reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenewOutput {
    /// The renewed placement.
    pub placement_id: PlacementId,
    /// The expiry before this renewal.
    pub previous_expires_at: u64,
    /// The new expiry.
    pub expires_at: u64,
}

/// A removal request: the publisher ends one pending placement early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveRequest {
    /// The placement to remove.
    pub placement_id: PlacementId,
    /// The publisher's replay identifier; the same value repeats the same removal.
    pub removal_id: RemovalId,
    /// The expiry the publisher believes is current.
    pub expected_expires_at: u64,
    /// The smallest early-release credit amount the publisher accepts.
    pub minimum_release_credits: u64,
}

/// What a completed removal reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveOutput {
    /// The removed placement.
    pub placement_id: PlacementId,
    /// The replay identifier of this removal.
    pub removal_id: RemovalId,
    /// Always true for a success.
    pub removed: bool,
    /// Early-release credits issued to the publisher.
    pub release_credits: u64,
    /// The removal charge.
    pub price: u64,
}

/// A byte-evidence request for one pending placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckRequest {
    /// The placement to challenge.
    pub placement_id: PlacementId,
    /// The zero-based content offset, below the content size.
    pub offset: u64,
}

/// One content byte served as evidence that the host still holds the publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckOutput {
    /// The challenged placement.
    pub placement_id: PlacementId,
    /// The challenged offset.
    pub offset: u64,
    /// The content byte at that offset (a ciphertext byte when restricted).
    pub value: u8,
}
