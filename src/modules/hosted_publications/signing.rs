//! The exact publication signing bytes, publisher signing and the signed fixtures' byte-for-byte reproduction.
//!
//! Callers: a publisher creating a publication, hosts validating a store and readers checking a fetched publication. The signature
//! covers the exact stored content: when the publication is restricted it covers the ciphertext. Integers are big-endian, IDs and
//! hashes their exact 32 bytes and the domain string ASCII without a terminator.
use super::{
    error::Error,
    ids::{ContentHash, PublicationId, Signature},
    metadata::{Kind, Metadata, Publication, Visibility},
};
use crate::modules::{
    cryptography::primitives::hash, identity::api::IdentityId, messaging::ports::Signer,
};

/// Domain string that starts every publication signing input.
pub const DOMAIN: &[u8] = b"CR2SE-PUBLICATION-V1";
/// Largest content a publication may carry in this finite profile: the same 16384-byte limit as a Messaging payload and a Storage
/// object, so the content fits the finite Services value budget (65536 bytes in all) together with its other fields.
pub const MAXIMUM_CONTENT_BYTES: usize = 1 << 14;
/// Largest signing input handled: the content bound plus the metadata with a maximum audience.
pub const MAXIMUM_SIGNING_BYTES: usize = MAXIMUM_CONTENT_BYTES + 32 * 65_536 + 256;

/// The publication signing bytes: `DOMAIN || publicationId || publisherId || kindCode || u64be(sequence) || u64be(publishedAt) ||
/// visibilityCode || u32be(audience count) || audience IDs || u64be(contentSize) || contentHash || content`.
pub fn signing_bytes(metadata: &Metadata, content: &[u8]) -> Vec<u8> {
    tracing::trace!("Building the publication signing bytes");
    let mut bytes = Vec::with_capacity(
        DOMAIN.len() + 32 * 3 + 30 + metadata.audience.len() * 32 + content.len(),
    );
    bytes.extend_from_slice(DOMAIN);
    bytes.extend_from_slice(&metadata.publication_id.0);
    bytes.extend_from_slice(&metadata.publisher_id.0);
    bytes.push(metadata.kind.code());
    bytes.extend_from_slice(&metadata.sequence.to_be_bytes());
    bytes.extend_from_slice(&metadata.published_at.to_be_bytes());
    bytes.push(metadata.visibility.code());
    bytes.extend_from_slice(&(metadata.audience.len() as u32).to_be_bytes());
    for reader in &metadata.audience {
        bytes.extend_from_slice(&reader.0);
    }
    bytes.extend_from_slice(&metadata.content_size.to_be_bytes());
    bytes.extend_from_slice(&metadata.content_hash.0);
    bytes.extend_from_slice(content);
    bytes
}

/// What a publisher chooses when it creates a publication.
#[derive(Debug, Clone)]
pub struct Draft {
    /// Publisher-generated random identifier.
    pub publication_id: PublicationId,
    /// Content kind.
    pub kind: Kind,
    /// Publisher-assigned sequence; zero for resources.
    pub sequence: u64,
    /// The publisher's claimed time in Unix seconds.
    pub published_at: u64,
    /// Public or restricted.
    pub visibility: Visibility,
    /// Ascending reader identities; empty when public.
    pub audience: Vec<IdentityId>,
}

/// Create a signed publication from final content (ciphertext when restricted), as a publisher does before storing it.
/// This only builds and signs; the rules of section 4 are enforced by `verify` (plan step 4), which a publisher may call too.
/// # Errors
/// `Invalid` for empty or oversized content; `Unavailable` when the signer cannot sign.
pub fn sign_publication(
    signer: &dyn Signer,
    draft: Draft,
    content: Vec<u8>,
) -> Result<Publication, Error> {
    tracing::trace!("Signing a publication for storage");
    if content.is_empty() || content.len() > MAXIMUM_CONTENT_BYTES {
        return Err(Error::Invalid);
    }
    let content_hash =
        ContentHash(hash::sha256(&content, content.len()).map_err(|_| Error::Invalid)?);
    let metadata = Metadata {
        publication_id: draft.publication_id,
        publisher_id: signer.identity(),
        kind: draft.kind,
        sequence: draft.sequence,
        published_at: draft.published_at,
        visibility: draft.visibility,
        audience: draft.audience,
        content_size: content.len() as u64,
        content_hash,
    };
    let publisher_signature: Signature = signer.sign(&signing_bytes(&metadata, &content))?;
    Ok(Publication {
        metadata,
        publisher_public_key: signer.public_key(),
        publisher_signature,
        content,
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::modules::cryptography::{
        credential,
        primitives::signature,
        secrets::{Secret, Signing},
    };
    use crate::modules::messaging::error::Error as MessagingError;
    use zeroize::Zeroizing;

    /// The independent vectors, parsed into sections of `label: hexadecimal` lines.
    pub(crate) struct Vectors(Vec<Section>);

    /// A change to metadata used by the mutation test.
    type Mutation = Box<dyn Fn(&mut Metadata)>;

    /// One vector section: its title and its `label: value` lines.
    type Section = (String, Vec<(String, String)>);

    impl Vectors {
        /// Parse the copied upstream vector file.
        pub(crate) fn load() -> Self {
            let text = include_str!("fixtures/vectors.md");
            // Lines before the first `==` heading are the shared inputs (keys and identities).
            let mut sections: Vec<(String, Vec<(String, String)>)> =
                vec![("inputs".to_string(), Vec::new())];
            for line in text.lines() {
                if let Some(title) = line.strip_prefix("== ") {
                    sections.push((title.to_string(), Vec::new()));
                } else if let (Some(section), Some((label, value))) =
                    (sections.last_mut(), line.split_once(": "))
                {
                    section.1.push((label.to_string(), value.to_string()));
                }
            }
            Self(sections)
        }

        /// The decoded value of `label` in the section whose title starts with `section`.
        pub(crate) fn bytes(&self, section: &str, label: &str) -> Vec<u8> {
            let (_, entries) = self.0.iter().find(|(t, _)| t.starts_with(section)).unwrap();
            let (_, value) = entries.iter().find(|(l, _)| l == label).unwrap();
            (0..value.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
                .collect()
        }
    }

    const SEED: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";

    /// The vectors' publisher: the RFC 8032 seed `9d61...7f60` with a real Ed25519 key.
    pub(crate) struct VectorPublisher {
        secret: Secret<Signing>,
        key: [u8; 32],
    }

    impl VectorPublisher {
        /// The publisher built from the vector seed.
        pub(crate) fn new() -> Self {
            // The seed is stated in the vector file's prose: RFC 8032 test 1.
            let seed: [u8; 32] = (0..32)
                .map(|i| u8::from_str_radix(&SEED[2 * i..2 * i + 2], 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            Self::from_seed(seed)
        }
        /// A publisher for an explicit seed.
        pub(crate) fn from_seed(seed: [u8; 32]) -> Self {
            let key = ed25519_dalek::SigningKey::from_bytes(&seed)
                .verifying_key()
                .to_bytes();
            Self {
                secret: Secret::new(Zeroizing::new(seed)),
                key,
            }
        }
    }

    impl Signer for VectorPublisher {
        // The identity derived from the public key.
        fn identity(&self) -> IdentityId {
            IdentityId(credential::identity_id(&self.key).unwrap())
        }
        // The vector publisher's public key.
        fn public_key(&self) -> [u8; 32] {
            self.key
        }
        // A real Ed25519 signature.
        fn sign(&self, message: &[u8]) -> Result<Signature, MessagingError> {
            signature::sign(&self.secret, message, MAXIMUM_SIGNING_BYTES)
                .map(Signature)
                .map_err(|_| MessagingError::Invalid)
        }
    }

    // The vector publication of a section: its draft values come from the vector file's inputs and the signing rules.
    fn vector_publication(publisher: &VectorPublisher, section: &str, draft: Draft) -> Publication {
        let vectors = Vectors::load();
        sign_publication(publisher, draft, vectors.bytes(section, "content")).unwrap()
    }

    fn id(vectors: &Vectors, section: &str, label: &str) -> [u8; 32] {
        vectors.bytes(section, label).try_into().unwrap()
    }

    // The three fixed publications (public resource, public page manifest, restricted feed-post envelope) produce exactly the
    // independent signing bytes, their SHA-256 and the publisher signature, and carry the vector publisher identity (HP-01).
    #[test]
    fn signing_reproduces_the_independent_vectors() {
        let vectors = Vectors::load();
        let publisher = VectorPublisher::new();
        let reader = IdentityId(id(&vectors, "inputs", "readerId (audience)"));
        assert_eq!(
            publisher.public_key().to_vec(),
            vectors.bytes("inputs", "publisherPublicKey")
        );
        assert_eq!(
            publisher.identity().0.to_vec(),
            vectors.bytes("inputs", "publisherId")
        );
        let cases = [
            ("V1", Kind::Resource, 0, 1000, Visibility::Public, vec![]),
            ("V2", Kind::Page, 1, 1001, Visibility::Public, vec![]),
            (
                "V3 restricted feed post (envelope)",
                Kind::FeedPost,
                7,
                1002,
                Visibility::Restricted,
                vec![reader],
            ),
        ];
        for (section, kind, sequence, published_at, visibility, audience) in cases {
            let draft = Draft {
                publication_id: PublicationId(id(&vectors, section, "publicationId")),
                kind,
                sequence,
                published_at,
                visibility,
                audience,
            };
            let publication = vector_publication(&publisher, section, draft);
            let bytes = signing_bytes(&publication.metadata, &publication.content);
            assert_eq!(bytes, vectors.bytes(section, "signingBytes"), "{section}");
            assert_eq!(
                hash::sha256(&bytes, bytes.len()).unwrap().to_vec(),
                vectors.bytes(section, "signingBytesSHA256"),
                "{section}"
            );
            assert_eq!(
                publication.publisher_signature.0.to_vec(),
                vectors.bytes(section, "publisherSignature"),
                "{section}"
            );
            assert_eq!(
                publication.metadata.content_hash.0.to_vec(),
                vectors.bytes(section, "contentHash")
            );
        }
    }

    // Changing any metadata field, the audience or a content byte changes the signing bytes, so a kept signature cannot cover it.
    #[test]
    fn every_mutation_changes_the_signing_bytes() {
        let vectors = Vectors::load();
        let publisher = VectorPublisher::new();
        let reader = IdentityId(id(&vectors, "inputs", "readerId (audience)"));
        let draft = Draft {
            publication_id: PublicationId([4; 32]),
            kind: Kind::FeedPost,
            sequence: 7,
            published_at: 1002,
            visibility: Visibility::Restricted,
            audience: vec![reader],
        };
        let base = sign_publication(&publisher, draft, b"hello".to_vec()).unwrap();
        let original = signing_bytes(&base.metadata, &base.content);
        let mutations: Vec<Mutation> = vec![
            Box::new(|m| m.publication_id.0[0] ^= 1),
            Box::new(|m| m.publisher_id.0[0] ^= 1),
            Box::new(|m| m.kind = Kind::Page),
            Box::new(|m| m.sequence += 1),
            Box::new(|m| m.published_at += 1),
            Box::new(|m| m.visibility = Visibility::Public),
            Box::new(|m| m.audience.clear()),
            Box::new(|m| m.audience[0].0[0] ^= 1),
            Box::new(|m| m.content_size += 1),
            Box::new(|m| m.content_hash.0[0] ^= 1),
        ];
        for mutate in mutations {
            let mut metadata = base.metadata.clone();
            mutate(&mut metadata);
            assert_ne!(signing_bytes(&metadata, &base.content), original);
        }
        assert_ne!(signing_bytes(&base.metadata, b"hellp"), original);
    }

    // Empty or oversized content is never signed, and a signer that cannot sign reports unavailable.
    #[test]
    fn signing_refuses_bad_content_and_missing_keys() {
        let publisher = VectorPublisher::new();
        let draft = || Draft {
            publication_id: PublicationId([1; 32]),
            kind: Kind::Resource,
            sequence: 0,
            published_at: 1,
            visibility: Visibility::Public,
            audience: vec![],
        };
        assert_eq!(
            sign_publication(&publisher, draft(), vec![]).unwrap_err(),
            Error::Invalid
        );
        assert_eq!(
            sign_publication(&publisher, draft(), vec![0; MAXIMUM_CONTENT_BYTES + 1]).unwrap_err(),
            Error::Invalid
        );
        assert!(sign_publication(&publisher, draft(), vec![0; MAXIMUM_CONTENT_BYTES]).is_ok());
        struct Broken;
        impl Signer for Broken {
            fn identity(&self) -> IdentityId {
                IdentityId([0; 32])
            }
            fn public_key(&self) -> [u8; 32] {
                [0; 32]
            }
            fn sign(&self, _: &[u8]) -> Result<Signature, MessagingError> {
                Err(MessagingError::Unavailable)
            }
        }
        assert_eq!(
            sign_publication(&Broken, draft(), vec![1]).unwrap_err(),
            Error::Unavailable
        );
    }
}
