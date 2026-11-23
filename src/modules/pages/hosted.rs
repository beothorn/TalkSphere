//! Hosted pages: building a page snapshot as publications, and assembling one on the reader side.
//!
//! Callers: the publisher tool and the reader. A hosted page is one `page` publication whose content is the [`Manifest`], plus one
//! `resource` publication per resource, all signed by the same publisher with the same visibility and audience. The reader fetches the
//! manifest, verifies it, then fetches each named resource from any host and checks that it is signed by the same publisher, has kind
//! `resource`, the same visibility and audience, the manifest's publication ID, size and hash, and (when restricted) decrypts. A resource that
//! fails any check is not part of the page; a resource no host can supply makes the page incomplete, which is not corruption.
//! For a restricted page the stored resource content is the envelope, so the manifest's size and hash describe the stored (encrypted) bytes.
use super::{
    error::Error,
    manifest::{Entry, Manifest},
    media::MediaType,
    path::CanonicalPath,
};
use crate::modules::{
    cryptography::primitives::hash,
    hosted_publications::{
        envelope,
        ids::{ContentHash, PublicationId},
        metadata::{Kind, Metadata, Publication, Visibility},
        signing::{Draft, sign_publication},
        verify::{Bounds, verify},
    },
    identity::api::IdentityId,
    messaging::{
        encryption::PersistentAgreement,
        ports::{Entropy, Signer},
    },
};
use std::collections::BTreeMap;

/// One page resource in plaintext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    /// The resource's canonical path.
    pub path: CanonicalPath,
    /// The resource's media type.
    pub media_type: MediaType,
    /// The exact resource bytes.
    pub content: Vec<u8>,
}

/// Who may read a page and the keys that encrypt it for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Audience {
    /// Plaintext, readable by every reader.
    Public,
    /// Encrypted for these readers, in strictly ascending identity order, each with its verified X25519 encryption key.
    Restricted(Vec<(IdentityId, [u8; 32])>),
}

/// What the publisher chooses for one page snapshot.
#[derive(Debug, Clone)]
pub struct PageDraft {
    /// The page's version number: a larger sequence replaces an older page.
    pub sequence: u64,
    /// The publisher's claimed time in Unix seconds.
    pub published_at: u64,
    /// Who may read the page.
    pub audience: Audience,
    /// The resources; they must include the root `/` as `text/html`.
    pub resources: Vec<Resource>,
}

/// The publications of one page snapshot, ready to store at hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    /// One publication per resource, in manifest (path) order.
    pub resources: Vec<Publication>,
    /// The manifest publication.
    pub page: Publication,
}

/// Build a page snapshot: encrypt and sign every resource, list them in a manifest and sign that.
/// # Errors
/// `Invalid` for empty content, a manifest that breaks the page rules, a restricted audience that is empty or unordered, or content above
/// the finite profile; `Unavailable` when randomness or the signer is missing.
pub fn build(
    signer: &dyn Signer,
    draft: PageDraft,
    maximum_resources: usize,
    entropy: &dyn Entropy,
) -> Result<Built, Error> {
    tracing::trace!("Building a hosted page snapshot");
    let (visibility, audience, keys): (Visibility, Vec<IdentityId>, Vec<[u8; 32]>) =
        match &draft.audience {
            Audience::Public => (Visibility::Public, vec![], vec![]),
            Audience::Restricted(readers) => (
                Visibility::Restricted,
                readers.iter().map(|(id, _)| *id).collect(),
                readers.iter().map(|(_, key)| *key).collect(),
            ),
        };
    let mut resources = Vec::with_capacity(draft.resources.len());
    let mut entries = Vec::with_capacity(draft.resources.len());
    for resource in &draft.resources {
        let mut id = [0u8; 32];
        entropy.fill(&mut id).map_err(|_| Error::Unavailable)?;
        let publication_id = PublicationId(id);
        let stored = stored_content(
            signer,
            publication_id,
            Kind::Resource,
            0,
            draft.published_at,
            visibility,
            &keys,
            &resource.content,
            entropy,
        )?;
        let publication = sign_publication(
            signer,
            Draft {
                publication_id,
                kind: Kind::Resource,
                sequence: 0,
                published_at: draft.published_at,
                visibility,
                audience: audience.clone(),
            },
            stored,
        )?;
        entries.push(Entry {
            path: resource.path.clone(),
            media_type: resource.media_type.clone(),
            size: publication.metadata.content_size,
            hash: publication.metadata.content_hash.0,
            publication_id,
        });
        resources.push((resource.path.clone(), publication));
    }
    let manifest = Manifest::new(entries, maximum_resources)?;
    resources.sort_by(|a, b| a.0.cmp(&b.0));
    let mut id = [0u8; 32];
    entropy.fill(&mut id).map_err(|_| Error::Unavailable)?;
    let page_id = PublicationId(id);
    let stored = stored_content(
        signer,
        page_id,
        Kind::Page,
        draft.sequence,
        draft.published_at,
        visibility,
        &keys,
        &manifest.encode(),
        entropy,
    )?;
    let page = sign_publication(
        signer,
        Draft {
            publication_id: page_id,
            kind: Kind::Page,
            sequence: draft.sequence,
            published_at: draft.published_at,
            visibility,
            audience,
        },
        stored,
    )?;
    Ok(Built {
        resources: resources.into_iter().map(|(_, p)| p).collect(),
        page,
    })
}

// The bytes a publication stores: the plaintext when public, the sealed envelope when restricted.
#[allow(clippy::too_many_arguments)]
fn stored_content(
    signer: &dyn Signer,
    publication_id: PublicationId,
    kind: Kind,
    sequence: u64,
    published_at: u64,
    visibility: Visibility,
    keys: &[[u8; 32]],
    plaintext: &[u8],
    entropy: &dyn Entropy,
) -> Result<Vec<u8>, Error> {
    if visibility == Visibility::Public {
        return Ok(plaintext.to_vec());
    }
    // Only the fields that the encryption metadata covers matter here; size and hash are not part of it.
    let metadata = Metadata {
        publication_id,
        publisher_id: signer.identity(),
        kind,
        sequence,
        published_at,
        visibility,
        audience: vec![],
        content_size: 0,
        content_hash: ContentHash([0; 32]),
    };
    Ok(envelope::seal(plaintext, &metadata, keys, entropy)?)
}

/// Where a reader obtains a resource publication: any host that holds it.
pub trait Fetch {
    /// The publication `id` of `publisher`, or `None` when no host could supply it.
    /// # Errors
    /// Only for a local failure; an absent publication is `Ok(None)`.
    fn publication(
        &self,
        publisher: IdentityId,
        id: PublicationId,
    ) -> Result<Option<Publication>, Error>;
}

/// What the reader brings: its limits and, for restricted pages, the private key it holds.
pub struct Reader<'a> {
    /// The verification limits (audience and content bounds).
    pub bounds: Bounds,
    /// The most resources a manifest may name.
    pub maximum_resources: usize,
    /// The reader's persistent agreement and the X25519 public key it holds, needed only to open restricted pages.
    pub key: Option<(&'a dyn PersistentAgreement, [u8; 32])>,
}

/// An assembled page snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// The verified publisher.
    pub publisher: IdentityId,
    /// The page's sequence.
    pub sequence: u64,
    /// The manifest publication's identifier.
    pub publication_id: PublicationId,
    /// The verified manifest.
    pub manifest: Manifest,
    /// The resources that passed every check.
    pub resources: BTreeMap<CanonicalPath, Resource>,
    /// Resources no host could supply: the page is incomplete.
    pub missing: Vec<CanonicalPath>,
    /// Resources that were supplied but failed a check and are not part of the page.
    pub rejected: Vec<CanonicalPath>,
}

impl Page {
    /// Whether every resource the manifest names was obtained and verified.
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty() && self.rejected.is_empty()
    }

    /// The verified resource at `path`, if it is part of the page.
    pub fn resource(&self, path: &CanonicalPath) -> Option<&Resource> {
        self.resources.get(path)
    }
}

/// Verify a manifest publication and assemble the page it describes from `fetch`.
/// # Errors
/// `Invalid` when the manifest publication fails verification, is not a `page`, or its manifest is malformed; `NotAvailable` when it is
/// restricted and the reader holds no key that opens it. Resource failures do not fail the assembly: they are reported in the page.
pub fn assemble(
    manifest_publication: &Publication,
    expected_publisher: IdentityId,
    reader: &Reader<'_>,
    fetch: &dyn Fetch,
) -> Result<Page, Error> {
    tracing::trace!("Assembling a hosted page");
    verify(manifest_publication, expected_publisher, reader.bounds)?;
    let metadata = &manifest_publication.metadata;
    if metadata.kind != Kind::Page {
        return Err(Error::Invalid);
    }
    let plaintext = plaintext_of(manifest_publication, reader)?;
    let manifest = Manifest::decode(&plaintext, reader.maximum_resources)?;
    let mut page = Page {
        publisher: expected_publisher,
        sequence: metadata.sequence,
        publication_id: metadata.publication_id,
        manifest: manifest.clone(),
        resources: BTreeMap::new(),
        missing: vec![],
        rejected: vec![],
    };
    for entry in manifest.entries() {
        let Some(publication) = fetch.publication(expected_publisher, entry.publication_id)? else {
            page.missing.push(entry.path.clone());
            continue;
        };
        match accepted_resource(&publication, entry, metadata, expected_publisher, reader) {
            Some(content) => {
                page.resources.insert(
                    entry.path.clone(),
                    Resource {
                        path: entry.path.clone(),
                        media_type: entry.media_type.clone(),
                        content,
                    },
                );
            }
            None => page.rejected.push(entry.path.clone()),
        }
    }
    Ok(page)
}

// The plaintext of a publication: its content when public, the opened envelope when restricted.
fn plaintext_of(publication: &Publication, reader: &Reader<'_>) -> Result<Vec<u8>, Error> {
    match publication.metadata.visibility {
        Visibility::Public => Ok(publication.content.clone()),
        Visibility::Restricted => {
            let (agreement, public_key) = reader.key.ok_or(Error::NotAvailable)?;
            envelope::open(
                &publication.content,
                &publication.metadata,
                agreement,
                &public_key,
            )
            .map(|plain| plain.to_vec())
            .map_err(|_| Error::NotAvailable)
        }
    }
}

// The resource bytes when the publication passes every check against its manifest entry and the manifest publication, otherwise `None`.
fn accepted_resource(
    publication: &Publication,
    entry: &Entry,
    page: &Metadata,
    publisher: IdentityId,
    reader: &Reader<'_>,
) -> Option<Vec<u8>> {
    let m = &publication.metadata;
    let same = m.kind == Kind::Resource
        && m.publisher_id == publisher
        && m.publication_id == entry.publication_id
        && m.visibility == page.visibility
        && m.audience == page.audience
        && m.content_size == entry.size
        && m.content_hash.0 == entry.hash;
    if !same || verify(publication, publisher, reader.bounds).is_err() {
        return None;
    }
    // The size and hash were checked by verify; the stored bytes must also hash to the manifest entry.
    let actual = hash::sha256(&publication.content, publication.content.len()).ok()?;
    if actual != entry.hash {
        return None;
    }
    plaintext_of(publication, reader).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{
        cryptography::secrets::{Persistent, Secret},
        hosted_publications::signing::tests::VectorPublisher,
        messaging::test_support::SequenceEntropy,
    };
    use std::cell::RefCell;
    use zeroize::Zeroizing;

    const MAXIMUM: usize = 8;

    // An in-memory set of hosts' publications, with a record of what was asked for.
    struct Hosts {
        held: RefCell<BTreeMap<PublicationId, Publication>>,
    }
    impl Fetch for Hosts {
        fn publication(
            &self,
            _: IdentityId,
            id: PublicationId,
        ) -> Result<Option<Publication>, Error> {
            Ok(self.held.borrow().get(&id).cloned())
        }
    }
    fn hosts(built: &Built) -> Hosts {
        let held = built
            .resources
            .iter()
            .map(|p| (p.metadata.publication_id, p.clone()))
            .collect();
        Hosts {
            held: RefCell::new(held),
        }
    }

    fn resource(path: &str, media: &str, content: &str) -> Resource {
        Resource {
            path: CanonicalPath::parse(path.as_bytes()).unwrap(),
            media_type: MediaType::parse(media).unwrap(),
            content: content.as_bytes().to_vec(),
        }
    }
    fn draft(audience: Audience) -> PageDraft {
        PageDraft {
            sequence: 3,
            published_at: 1000,
            audience,
            resources: vec![
                resource("/", "text/html", "<h1>hi</h1>"),
                resource("/style.css", "text/css", "h1{color:red}"),
                resource("/img/a.txt", "text/plain", "x"),
            ],
        }
    }
    fn reader<'a>(key: Option<(&'a dyn PersistentAgreement, [u8; 32])>) -> Reader<'a> {
        Reader {
            bounds: Bounds {
                maximum_audience: 4,
                maximum_content_bytes: 16384,
            },
            maximum_resources: MAXIMUM,
            key,
        }
    }

    // HP-08: a public page assembles completely: the manifest verifies, the root is HTML and every resource matches its entry.
    #[test]
    fn a_public_page_assembles_completely() {
        let publisher = VectorPublisher::new();
        let built = build(
            &publisher,
            draft(Audience::Public),
            MAXIMUM,
            &SequenceEntropy::new(1),
        )
        .unwrap();
        assert_eq!(built.resources.len(), 3);
        let page = assemble(
            &built.page,
            publisher.identity(),
            &reader(None),
            &hosts(&built),
        )
        .unwrap();
        assert!(page.is_complete());
        assert_eq!((page.sequence, page.resources.len()), (3, 3));
        let root = page.resource(&CanonicalPath::root()).unwrap();
        assert_eq!(
            (root.media_type.as_str(), root.content.as_slice()),
            ("text/html", &b"<h1>hi</h1>"[..])
        );
        assert_eq!(
            page.resource(&CanonicalPath::parse(b"/style.css").unwrap())
                .unwrap()
                .content,
            b"h1{color:red}"
        );
    }

    // HP-08: a resource that is wrong in any way is not part of the page, whatever a host serves: another publisher, another kind,
    // another visibility, a different size or hash, a damaged signature or another publication ID.
    #[test]
    fn resources_that_fail_a_check_are_rejected() {
        let publisher = VectorPublisher::new();
        let built = build(
            &publisher,
            draft(Audience::Public),
            MAXIMUM,
            &SequenceEntropy::new(1),
        )
        .unwrap();
        let target = built.resources[1].clone();
        let id = target.metadata.publication_id;
        let stranger = VectorPublisher::from_seed([7; 32]);
        let other = crate::modules::hosted_publications::signing::sign_publication(
            &stranger,
            Draft {
                publication_id: id,
                kind: Kind::Resource,
                sequence: 0,
                published_at: 1000,
                visibility: Visibility::Public,
                audience: vec![],
            },
            target.content.clone(),
        )
        .unwrap();
        let resign = |change: &dyn Fn(&mut Metadata, &mut Vec<u8>)| {
            let mut metadata = target.metadata.clone();
            let mut content = target.content.clone();
            change(&mut metadata, &mut content);
            crate::modules::hosted_publications::signing::sign_publication(
                &publisher,
                Draft {
                    publication_id: metadata.publication_id,
                    kind: metadata.kind,
                    sequence: metadata.sequence,
                    published_at: metadata.published_at,
                    visibility: metadata.visibility,
                    audience: metadata.audience.clone(),
                },
                content,
            )
            .unwrap()
        };
        let mut bad_signature = target.clone();
        bad_signature.publisher_signature.0[0] ^= 1;
        let mut tampered = target.clone();
        tampered.content[0] ^= 1;
        let candidates = vec![
            other,
            resign(&|m, _| m.kind = Kind::FeedPost),
            resign(&|m, _| {
                m.visibility = Visibility::Restricted;
                m.audience = vec![IdentityId([9; 32])];
            }),
            resign(&|_, c| c.push(b'!')),
            resign(&|_, c| c[0] ^= 1),
            bad_signature,
            tampered,
        ];
        for candidate in candidates {
            let hosts = hosts(&built);
            hosts.held.borrow_mut().insert(id, candidate);
            let page = assemble(&built.page, publisher.identity(), &reader(None), &hosts).unwrap();
            assert!(!page.is_complete());
            assert_eq!(
                page.rejected,
                vec![CanonicalPath::parse(b"/img/a.txt").unwrap()]
            );
            assert_eq!(page.resources.len(), 2);
            assert!(page.missing.is_empty());
        }
    }

    // A resource that no host holds makes the page incomplete (not corrupt), and what is held is still shown.
    #[test]
    fn a_missing_resource_makes_the_page_incomplete() {
        let publisher = VectorPublisher::new();
        let built = build(
            &publisher,
            draft(Audience::Public),
            MAXIMUM,
            &SequenceEntropy::new(1),
        )
        .unwrap();
        let hosts = hosts(&built);
        hosts
            .held
            .borrow_mut()
            .remove(&built.resources[0].metadata.publication_id);
        let page = assemble(&built.page, publisher.identity(), &reader(None), &hosts).unwrap();
        assert_eq!(
            (
                page.is_complete(),
                page.missing.len(),
                page.rejected.len(),
                page.resources.len()
            ),
            (false, 1, 0, 2)
        );
    }

    // HP-08: the manifest itself must verify and be a page: a forged or foreign manifest, a wrong kind or a malformed manifest is refused.
    #[test]
    fn a_bad_manifest_publication_is_refused() {
        let publisher = VectorPublisher::new();
        let built = build(
            &publisher,
            draft(Audience::Public),
            MAXIMUM,
            &SequenceEntropy::new(1),
        )
        .unwrap();
        let ask = |publication: &Publication, who: IdentityId| {
            assemble(publication, who, &reader(None), &hosts(&built))
        };
        assert!(ask(&built.page, publisher.identity()).is_ok());
        assert_eq!(
            ask(&built.page, IdentityId([1; 32])).err(),
            Some(Error::Invalid)
        );
        let mut forged = built.page.clone();
        forged.content[40] ^= 1;
        assert_eq!(
            ask(&forged, publisher.identity()).err(),
            Some(Error::Invalid)
        );
        // A resource publication is not a page, and a signed page whose content is not a manifest is malformed.
        assert_eq!(
            ask(&built.resources[0], publisher.identity()).err(),
            Some(Error::Invalid)
        );
        let junk = crate::modules::hosted_publications::signing::sign_publication(
            &publisher,
            Draft {
                publication_id: PublicationId([4; 32]),
                kind: Kind::Page,
                sequence: 1,
                published_at: 1,
                visibility: Visibility::Public,
                audience: vec![],
            },
            b"not a manifest".to_vec(),
        )
        .unwrap();
        assert_eq!(ask(&junk, publisher.identity()).err(), Some(Error::Invalid));
    }

    // A restricted page is encrypted for its audience: an audience member assembles it, a stranger with another key cannot, and a reader
    // with no key learns nothing; the manifest's size and hash describe the stored encrypted bytes.
    #[test]
    fn a_restricted_page_opens_only_for_its_audience() {
        let publisher = VectorPublisher::new();
        let key_of = |n: u8| {
            x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from([n; 32])).to_bytes()
        };
        let secret_of = |n: u8| Secret::<Persistent>::new(Zeroizing::new([n; 32]));
        let mut readers = vec![
            (IdentityId([1; 32]), key_of(1)),
            (IdentityId([2; 32]), key_of(2)),
        ];
        readers.sort_by_key(|(id, _)| id.0);
        let built = build(
            &publisher,
            draft(Audience::Restricted(readers)),
            MAXIMUM,
            &SequenceEntropy::new(1),
        )
        .unwrap();
        assert!(built.page.metadata.visibility == Visibility::Restricted);
        for p in &built.resources {
            assert_ne!(p.content, b"<h1>hi</h1>");
        }
        let mine = secret_of(2);
        let page = assemble(
            &built.page,
            publisher.identity(),
            &reader(Some((&mine, key_of(2)))),
            &hosts(&built),
        )
        .unwrap();
        assert!(page.is_complete());
        assert_eq!(
            page.resource(&CanonicalPath::root()).unwrap().content,
            b"<h1>hi</h1>"
        );
        let stranger = secret_of(9);
        assert_eq!(
            assemble(
                &built.page,
                publisher.identity(),
                &reader(Some((&stranger, key_of(9)))),
                &hosts(&built)
            )
            .err(),
            Some(Error::NotAvailable)
        );
        assert_eq!(
            assemble(
                &built.page,
                publisher.identity(),
                &reader(None),
                &hosts(&built)
            )
            .err(),
            Some(Error::NotAvailable)
        );
        // A damaged encrypted resource is rejected, not shown.
        let damaged = hosts(&built);
        let id = built.resources[0].metadata.publication_id;
        let mut broken = built.resources[0].clone();
        let last = broken.content.len() - 1;
        broken.content[last] ^= 1;
        damaged.held.borrow_mut().insert(id, broken);
        let page = assemble(
            &built.page,
            publisher.identity(),
            &reader(Some((&mine, key_of(2)))),
            &damaged,
        )
        .unwrap();
        assert_eq!(page.rejected.len(), 1);
    }

    // Building refuses a page that breaks the rules (no root, a root that is not HTML, no resources, empty content, too many), and missing
    // randomness is unavailable.
    #[test]
    fn building_refuses_invalid_pages() {
        let publisher = VectorPublisher::new();
        let entropy = SequenceEntropy::new(1);
        let mut no_root = draft(Audience::Public);
        no_root.resources.remove(0);
        assert_eq!(
            build(&publisher, no_root, MAXIMUM, &entropy).err(),
            Some(Error::Invalid)
        );
        let mut not_html = draft(Audience::Public);
        not_html.resources[0].media_type = MediaType::parse("text/plain").unwrap();
        assert_eq!(
            build(&publisher, not_html, MAXIMUM, &entropy).err(),
            Some(Error::Invalid)
        );
        let mut empty = draft(Audience::Public);
        empty.resources[1].content.clear();
        assert_eq!(
            build(&publisher, empty, MAXIMUM, &entropy).err(),
            Some(Error::Invalid)
        );
        let mut none = draft(Audience::Public);
        none.resources.clear();
        assert_eq!(
            build(&publisher, none, MAXIMUM, &entropy).err(),
            Some(Error::Invalid)
        );
        assert_eq!(
            build(&publisher, draft(Audience::Public), 2, &entropy).err(),
            Some(Error::Invalid)
        );
        assert_eq!(
            build(
                &publisher,
                draft(Audience::Restricted(vec![])),
                MAXIMUM,
                &entropy
            )
            .err(),
            Some(Error::Invalid)
        );
        entropy.fail();
        assert_eq!(
            build(&publisher, draft(Audience::Public), MAXIMUM, &entropy).err(),
            Some(Error::Unavailable)
        );
    }
}
