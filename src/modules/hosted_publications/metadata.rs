//! Publication metadata and the signed publication value.
//!
//! Callers: signing, verification, placements and the provider operations. A publication is a signed association between this
//! metadata and one immutable content byte sequence. Metadata other than the audience is visible to hosts; encryption protects only
//! the content (see the module guide).
use super::{
    error::Error,
    ids::{ContentHash, PublicationId, Signature},
};
use crate::modules::identity::api::IdentityId;

/// What a publication's content is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Content is a page manifest.
    Page,
    /// Content is a feed post.
    FeedPost,
    /// Content is one page resource's bytes.
    Resource,
}

impl Kind {
    /// The one-byte code used in signing bytes: page 1, feed post 2, resource 3.
    pub fn code(self) -> u8 {
        match self {
            Self::Page => 1,
            Self::FeedPost => 2,
            Self::Resource => 3,
        }
    }

    /// The wire name: `page`, `feed-post` or `resource`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::FeedPost => "feed-post",
            Self::Resource => "resource",
        }
    }

    /// Decode a wire name.
    /// # Errors
    /// `Unsupported` for any other text.
    pub fn from_name(name: &str) -> Result<Self, Error> {
        tracing::trace!("Decoding a publication kind name");
        match name {
            "page" => Ok(Self::Page),
            "feed-post" => Ok(Self::FeedPost),
            "resource" => Ok(Self::Resource),
            _ => Err(Error::Unsupported),
        }
    }

    /// Whether readers may list this kind: pages and feed posts form streams, resources are addressed only by ID.
    pub fn is_listed(self) -> bool {
        !matches!(self, Self::Resource)
    }
}

/// Who may read a publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// Plaintext content readable by every authenticated reader.
    Public,
    /// Encrypted content readable by the audience and the publisher.
    Restricted,
}

impl Visibility {
    /// The one-byte code used in signing bytes: public 0, restricted 1.
    pub fn code(self) -> u8 {
        match self {
            Self::Public => 0,
            Self::Restricted => 1,
        }
    }

    /// The wire name: `public` or `restricted`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Restricted => "restricted",
        }
    }

    /// Decode a wire name.
    /// # Errors
    /// `Unsupported` for any other text.
    pub fn from_name(name: &str) -> Result<Self, Error> {
        tracing::trace!("Decoding a publication visibility name");
        match name {
            "public" => Ok(Self::Public),
            "restricted" => Ok(Self::Restricted),
            _ => Err(Error::Unsupported),
        }
    }
}

/// The signed metadata of one publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    /// Publisher-generated identifier, identical at every host.
    pub publication_id: PublicationId,
    /// The publisher's identity.
    pub publisher_id: IdentityId,
    /// Content kind.
    pub kind: Kind,
    /// Publisher-assigned ordering and version number; zero for resources.
    pub sequence: u64,
    /// The publisher's claimed time in Unix seconds; not a trusted clock.
    pub published_at: u64,
    /// Public or restricted.
    pub visibility: Visibility,
    /// Reader identities of a restricted publication in strictly ascending order; empty when public.
    pub audience: Vec<IdentityId>,
    /// Content length in bytes, at least one.
    pub content_size: u64,
    /// SHA-256 of the content.
    pub content_hash: ContentHash,
}

/// A complete signed publication as stored and served by hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publication {
    /// The signed metadata.
    pub metadata: Metadata,
    /// The publisher's Ed25519 public key; its identity ID must equal `metadata.publisher_id`.
    pub publisher_public_key: [u8; 32],
    /// The publisher's signature over the signing bytes.
    pub publisher_signature: Signature,
    /// The exact content bytes (ciphertext when restricted).
    pub content: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Kinds and visibilities keep their specified codes and names, round trip through names and refuse unknown text.
    #[test]
    fn codes_and_names_are_exact() {
        for (kind, code, name) in [
            (Kind::Page, 1, "page"),
            (Kind::FeedPost, 2, "feed-post"),
            (Kind::Resource, 3, "resource"),
        ] {
            assert_eq!((kind.code(), kind.name()), (code, name));
            assert_eq!(Kind::from_name(name), Ok(kind));
        }
        assert_eq!(Kind::from_name("Page"), Err(Error::Unsupported));
        assert!(Kind::Page.is_listed() && !Kind::Resource.is_listed());
        assert_eq!(
            (Visibility::Public.code(), Visibility::Restricted.code()),
            (0, 1)
        );
        assert_eq!(
            Visibility::from_name("restricted"),
            Ok(Visibility::Restricted)
        );
        assert_eq!(Visibility::from_name(""), Err(Error::Unsupported));
    }
}
