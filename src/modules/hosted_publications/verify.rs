//! Publication verification: the checks a host performs before accepting a store and a reader performs before using a fetched
//! publication.
//!
//! Callers: the provider's store operation, the reader-side page assembly and tests. The checks run in the specified order and a
//! publication is never partly accepted: the publisher ID derives from the public key; it equals the authenticated or intended
//! publisher; kind, sequence, visibility, audience, size and hash are valid; the Ed25519 signature verifies over the exact signing
//! bytes. Restricted envelope structure is checked separately by `envelope` (plan step 5).
use super::{
    error::Error,
    metadata::{Kind, Publication, Visibility},
    signing::{MAXIMUM_CONTENT_BYTES, MAXIMUM_SIGNING_BYTES, signing_bytes},
};
use crate::modules::{
    cryptography::{
        credential,
        primitives::{hash, signature},
    },
    identity::api::IdentityId,
};

/// Bounds a verifier applies to a publication: the offering's limits for a host, the reader's own for a reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Most reader identities a restricted publication may name; zero allows public publications only.
    pub maximum_audience: usize,
    /// Largest content in bytes accepted; never above the finite profile's [`MAXIMUM_CONTENT_BYTES`].
    pub maximum_content_bytes: u64,
}

impl Default for Bounds {
    /// The finite profile's limits: 64 readers and the logical content budget.
    fn default() -> Self {
        Self {
            maximum_audience: 64,
            maximum_content_bytes: MAXIMUM_CONTENT_BYTES as u64,
        }
    }
}

/// Verify a signed publication.
///
/// `expected_publisher` is the authenticated requester when a host verifies a store, or the publisher a reader intended to read.
/// # Errors
/// `Invalid` for the first check that fails. The error names no field, so a caller cannot probe which rule rejected a value.
pub fn verify(
    publication: &Publication,
    expected_publisher: IdentityId,
    bounds: Bounds,
) -> Result<(), Error> {
    tracing::trace!("Verifying a signed publication");
    let metadata = &publication.metadata;
    let derived =
        credential::identity_id(&publication.publisher_public_key).map_err(|_| Error::Invalid)?;
    if derived != metadata.publisher_id.0 || metadata.publisher_id != expected_publisher {
        return Err(Error::Invalid);
    }
    check_rules(publication, bounds)?;
    let bytes = signing_bytes(metadata, &publication.content);
    signature::verify(
        &publication.publisher_public_key,
        &bytes,
        &publication.publisher_signature.0,
        MAXIMUM_SIGNING_BYTES,
    )
    .map_err(|_| Error::Invalid)
}

// Kind, sequence, visibility, audience, size and hash rules that need no key.
fn check_rules(publication: &Publication, bounds: Bounds) -> Result<(), Error> {
    tracing::trace!("Checking publication metadata rules");
    let metadata = &publication.metadata;
    if metadata.kind == Kind::Resource && metadata.sequence != 0 {
        return Err(Error::Invalid);
    }
    match metadata.visibility {
        Visibility::Public if !metadata.audience.is_empty() => return Err(Error::Invalid),
        Visibility::Restricted
            if metadata.audience.is_empty()
                || metadata.audience.len() > bounds.maximum_audience =>
        {
            return Err(Error::Invalid);
        }
        _ => {}
    }
    if metadata
        .audience
        .windows(2)
        .any(|pair| pair[0].0 >= pair[1].0)
    {
        return Err(Error::Invalid);
    }
    let size = publication.content.len() as u64;
    let largest = bounds
        .maximum_content_bytes
        .min(MAXIMUM_CONTENT_BYTES as u64);
    if size == 0 || size != metadata.content_size || size > largest {
        return Err(Error::Invalid);
    }
    let actual = hash::sha256(&publication.content, publication.content.len())
        .map_err(|_| Error::Invalid)?;
    if actual != metadata.content_hash.0 {
        return Err(Error::Invalid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::hosted_publications::{
        ids::PublicationId,
        metadata::Metadata,
        signing::{Draft, sign_publication, tests::VectorPublisher},
    };
    use crate::modules::messaging::ports::Signer;

    // A valid public page publication signed by the vector publisher, plus the publisher.
    fn valid(visibility: Visibility, audience: Vec<IdentityId>) -> (Publication, VectorPublisher) {
        let publisher = VectorPublisher::new();
        let draft = Draft {
            publication_id: PublicationId([5; 32]),
            kind: Kind::Page,
            sequence: 3,
            published_at: 1000,
            visibility,
            audience,
        };
        (
            sign_publication(&publisher, draft, b"content".to_vec()).unwrap(),
            publisher,
        )
    }

    // Re-sign a changed publication so only the rule under test, not the signature, can reject it.
    fn resign(
        publisher: &VectorPublisher,
        publication: &mut Publication,
        change: impl FnOnce(&mut Metadata),
    ) {
        change(&mut publication.metadata);
        let bytes = signing_bytes(&publication.metadata, &publication.content);
        publication.publisher_signature = publisher.sign(&bytes).unwrap();
    }

    // A correctly signed public and restricted publication verifies for its publisher, in both cases.
    #[test]
    fn valid_publications_verify() {
        let (publication, publisher) = valid(Visibility::Public, vec![]);
        assert_eq!(
            verify(&publication, publisher.identity(), Bounds::default()),
            Ok(())
        );
        let (restricted, publisher) = valid(Visibility::Restricted, vec![IdentityId([9; 32])]);
        assert_eq!(
            verify(&restricted, publisher.identity(), Bounds::default()),
            Ok(())
        );
    }

    // HP-03: a wrong or unexpected publisher, a damaged signature, mutated metadata or content, a wrong size or hash each fail.
    #[test]
    fn identity_signature_size_and_hash_failures() {
        let (publication, publisher) = valid(Visibility::Public, vec![]);
        let me = publisher.identity();
        assert_eq!(
            verify(&publication, IdentityId([1; 32]), Bounds::default()),
            Err(Error::Invalid)
        );
        let mut other_key = publication.clone();
        other_key.publisher_public_key[0] ^= 1;
        assert_eq!(
            verify(&other_key, me, Bounds::default()),
            Err(Error::Invalid)
        );
        let mut bad_signature = publication.clone();
        bad_signature.publisher_signature.0[0] ^= 1;
        assert_eq!(
            verify(&bad_signature, me, Bounds::default()),
            Err(Error::Invalid)
        );
        let mut bad_content = publication.clone();
        bad_content.content[0] ^= 1;
        assert_eq!(
            verify(&bad_content, me, Bounds::default()),
            Err(Error::Invalid)
        );
        let mut short = publication.clone();
        short.metadata.content_size += 1;
        assert_eq!(verify(&short, me, Bounds::default()), Err(Error::Invalid));
        let mut bad_hash = publication.clone();
        bad_hash.metadata.content_hash.0[0] ^= 1;
        assert_eq!(
            verify(&bad_hash, me, Bounds::default()),
            Err(Error::Invalid)
        );
        let mut changed = publication;
        changed.metadata.sequence += 1;
        assert_eq!(verify(&changed, me, Bounds::default()), Err(Error::Invalid));
    }

    // HP-03: rule violations are rejected even when correctly signed: resource sequence, audience shape and size bounds.
    #[test]
    fn signed_rule_violations_are_rejected() {
        let bounds = Bounds::default();
        let (mut resource, publisher) = valid(Visibility::Public, vec![]);
        resign(&publisher, &mut resource, |m| {
            m.kind = Kind::Resource;
            m.sequence = 1;
        });
        assert_eq!(
            verify(&resource, publisher.identity(), bounds),
            Err(Error::Invalid)
        );
        resign(&publisher, &mut resource, |m| m.sequence = 0);
        assert_eq!(verify(&resource, publisher.identity(), bounds), Ok(()));

        let (mut public, publisher) = valid(Visibility::Public, vec![]);
        resign(&publisher, &mut public, |m| {
            m.audience = vec![IdentityId([1; 32])]
        });
        assert_eq!(
            verify(&public, publisher.identity(), bounds),
            Err(Error::Invalid)
        );

        let (mut restricted, publisher) = valid(Visibility::Restricted, vec![IdentityId([1; 32])]);
        for audience in [
            vec![],
            vec![IdentityId([2; 32]), IdentityId([1; 32])],
            vec![IdentityId([1; 32]), IdentityId([1; 32])],
        ] {
            resign(&publisher, &mut restricted, |m| m.audience = audience);
            assert_eq!(
                verify(&restricted, publisher.identity(), bounds),
                Err(Error::Invalid)
            );
        }
        resign(&publisher, &mut restricted, |m| {
            m.audience = vec![IdentityId([1; 32]), IdentityId([2; 32])];
        });
        let tight = Bounds {
            maximum_audience: 1,
            ..bounds
        };
        assert_eq!(
            verify(&restricted, publisher.identity(), tight),
            Err(Error::Invalid)
        );
        let public_only = Bounds {
            maximum_audience: 0,
            ..bounds
        };
        assert_eq!(
            verify(&restricted, publisher.identity(), public_only),
            Err(Error::Invalid)
        );
        let small = Bounds {
            maximum_content_bytes: 3,
            ..bounds
        };
        assert_eq!(
            verify(&restricted, publisher.identity(), small),
            Err(Error::Invalid)
        );
    }

    // Empty content is refused even when size and hash are consistent and signed, and the profile bound caps any larger offering.
    #[test]
    fn empty_and_profile_oversized_content_are_refused() {
        let (mut publication, publisher) = valid(Visibility::Public, vec![]);
        publication.content.clear();
        resign(&publisher, &mut publication, |m| m.content_size = 0);
        assert_eq!(
            verify(&publication, publisher.identity(), Bounds::default()),
            Err(Error::Invalid)
        );
        // A correctly signed value one byte over the profile budget is refused even by an offering that allows more.
        let (mut big, publisher) = valid(Visibility::Public, vec![]);
        big.content = vec![7; MAXIMUM_CONTENT_BYTES + 1];
        let digest = hash::sha256(&big.content, big.content.len()).unwrap();
        resign(&publisher, &mut big, |m| {
            m.content_size = MAXIMUM_CONTENT_BYTES as u64 + 1;
            m.content_hash.0 = digest;
        });
        let huge = Bounds {
            maximum_audience: 1,
            maximum_content_bytes: u64::MAX,
        };
        assert_eq!(
            verify(&big, publisher.identity(), huge),
            Err(Error::Invalid)
        );
    }
}
