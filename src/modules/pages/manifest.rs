//! The canonical hosted page manifest: the signed list of a page snapshot's resources.
//!
//! Callers: the publisher building a hosted page and the reader assembling one. A page is a `page` publication whose content is this
//! manifest; each resource it names is a separate `resource` publication by the same publisher. The manifest lists every resource's
//! canonical path, media type, size, SHA-256 and publication ID, ordered by path bytes ascending without duplicates. It must name the root
//! path `/` with media type `text/html`. Decoding is strict and bounded: counts and lengths are checked before anything is allocated, and
//! trailing bytes are refused.
use super::{error::Error, media::MediaType, path::CanonicalPath};
use crate::modules::hosted_publications::ids::PublicationId;

/// Domain string that starts the canonical manifest bytes.
pub const DOMAIN: &[u8] = b"CR2SE-HOSTED-PAGE-MANIFEST-V1";
/// Largest path size in bytes accepted in this finite profile.
pub const MAXIMUM_PATH_BYTES: usize = 1024;
/// Largest media type size in bytes (two 63-character parts and a slash).
pub const MAXIMUM_MEDIA_TYPE_BYTES: usize = 127;
// A manifest entry's fixed part: the size, the hash and the publication ID.
const FIXED_ENTRY_BYTES: usize = 8 + 32 + 32;

/// One resource the manifest names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The resource's canonical path within the page.
    pub path: CanonicalPath,
    /// The resource's media type.
    pub media_type: MediaType,
    /// The size in bytes of the resource publication's content.
    pub size: u64,
    /// The SHA-256 of the resource publication's content.
    pub hash: [u8; 32],
    /// The publication that carries the resource.
    pub publication_id: PublicationId,
}

/// A validated manifest: between one and the allowed number of entries, ordered by path bytes ascending without duplicates, containing the
/// root path `/` with media type `text/html`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    entries: Vec<Entry>,
}

impl Manifest {
    /// Build a manifest from entries in any order: they are sorted by path, and the rules of a decoded manifest are enforced.
    /// # Errors
    /// `Invalid` for no entries, more than `maximum_resources`, a duplicate path, a path above the profile size, a missing root or a root
    /// that is not `text/html`.
    pub fn new(mut entries: Vec<Entry>, maximum_resources: usize) -> Result<Self, Error> {
        tracing::trace!("Building a page manifest");
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let manifest = Self { entries };
        manifest.check(maximum_resources)?;
        Ok(manifest)
    }

    /// The entries in canonical order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The entry at `path`, if the manifest names it.
    pub fn entry(&self, path: &CanonicalPath) -> Option<&Entry> {
        tracing::trace!("Looking up a manifest entry");
        self.entries
            .binary_search_by(|entry| entry.path.cmp(path))
            .ok()
            .map(|index| &self.entries[index])
    }

    /// The canonical manifest bytes: `DOMAIN || u32be(count) ||` for each entry `u64be(len) path || u64be(len) mediaType || u64be(size) ||
    /// hash || publicationId`.
    pub fn encode(&self) -> Vec<u8> {
        tracing::trace!("Encoding the canonical manifest bytes");
        let mut bytes = Vec::from(DOMAIN);
        bytes.extend_from_slice(&(self.entries.len() as u32).to_be_bytes());
        for entry in &self.entries {
            put_string(&mut bytes, entry.path.as_str());
            put_string(&mut bytes, entry.media_type.as_str());
            bytes.extend_from_slice(&entry.size.to_be_bytes());
            bytes.extend_from_slice(&entry.hash);
            bytes.extend_from_slice(&entry.publication_id.0);
        }
        bytes
    }

    /// Decode and validate canonical manifest bytes.
    /// # Errors
    /// `Invalid` for a wrong domain, a count of zero or above `maximum_resources`, a truncated or oversized string or entry, an invalid path or
    /// media type, entries out of order or duplicated, a missing root or a root that is not `text/html`, or trailing bytes.
    pub fn decode(bytes: &[u8], maximum_resources: usize) -> Result<Self, Error> {
        tracing::trace!("Decoding the canonical manifest bytes");
        let mut cursor = Cursor(bytes);
        if cursor.take(DOMAIN.len())? != DOMAIN {
            return Err(Error::Invalid);
        }
        let count = u32::from_be_bytes(cursor.take(4)?.try_into().map_err(|_| Error::Invalid)?);
        let count = usize::try_from(count).map_err(|_| Error::Invalid)?;
        // The smallest entry is a one-byte path and the shortest media type, so the count cannot exceed what the bytes could hold.
        if count == 0
            || count > maximum_resources
            || count > bytes.len() / (8 + 1 + 8 + 3 + FIXED_ENTRY_BYTES)
        {
            return Err(Error::Invalid);
        }
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let path = CanonicalPath::parse(cursor.string(MAXIMUM_PATH_BYTES)?)?;
            let media_type = MediaType::parse(
                std::str::from_utf8(cursor.string(MAXIMUM_MEDIA_TYPE_BYTES)?)
                    .map_err(|_| Error::Invalid)?,
            )?;
            entries.push(Entry {
                path,
                media_type,
                size: u64::from_be_bytes(cursor.take(8)?.try_into().map_err(|_| Error::Invalid)?),
                hash: cursor.take(32)?.try_into().map_err(|_| Error::Invalid)?,
                publication_id: PublicationId(
                    cursor.take(32)?.try_into().map_err(|_| Error::Invalid)?,
                ),
            });
        }
        if !cursor.0.is_empty() {
            return Err(Error::Invalid);
        }
        let manifest = Self { entries };
        manifest.check(maximum_resources)?;
        Ok(manifest)
    }

    // The rules shared by building and decoding.
    fn check(&self, maximum_resources: usize) -> Result<(), Error> {
        let entries = &self.entries;
        if entries.is_empty()
            || entries.len() > maximum_resources
            || entries.len() > u32::MAX as usize
            || entries.windows(2).any(|pair| pair[0].path >= pair[1].path)
            || entries.iter().any(|e| e.path.size() > MAXIMUM_PATH_BYTES)
        {
            return Err(Error::Invalid);
        }
        match entries.iter().find(|e| e.path.is_root()) {
            Some(root) if root.media_type.is_html() => Ok(()),
            _ => Err(Error::Invalid),
        }
    }
}

// A length-prefixed string: `u64be(byte length) || UTF-8 bytes`.
fn put_string(bytes: &mut Vec<u8>, text: &str) {
    bytes.extend_from_slice(&(text.len() as u64).to_be_bytes());
    bytes.extend_from_slice(text.as_bytes());
}

// A bounded reader over the manifest bytes.
struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    // The next `count` bytes, or `Invalid` when fewer remain.
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        if self.0.len() < count {
            return Err(Error::Invalid);
        }
        let (head, rest) = self.0.split_at(count);
        self.0 = rest;
        Ok(head)
    }
    // A length-prefixed string of at most `maximum` bytes; the length is compared with the remaining bytes before anything is taken.
    fn string(&mut self, maximum: usize) -> Result<&'a [u8], Error> {
        let length = u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Invalid)?);
        let length = usize::try_from(length).map_err(|_| Error::Invalid)?;
        if length > maximum {
            return Err(Error::Invalid);
        }
        self.take(length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::hosted_publications::signing::tests::Vectors;

    fn entry(path: &str, media: &str, id: u8) -> Entry {
        Entry {
            path: CanonicalPath::parse(path.as_bytes()).unwrap(),
            media_type: MediaType::parse(media).unwrap(),
            size: 12,
            hash: [id; 32],
            publication_id: PublicationId([id; 32]),
        }
    }

    // HP-08/V2: the independent page manifest vector is reproduced byte for byte from its resource and decodes back to the same entry.
    #[test]
    fn the_vector_manifest_is_reproduced() {
        let vectors = Vectors::load();
        let resource = vectors.bytes("V1", "content");
        let hash: [u8; 32] = vectors.bytes("V1", "contentHash").try_into().unwrap();
        let id: [u8; 32] = vectors.bytes("V1", "publicationId").try_into().unwrap();
        assert_eq!(resource.len(), 12);
        let manifest = Manifest::new(
            vec![Entry {
                path: CanonicalPath::root(),
                media_type: MediaType::parse("text/html").unwrap(),
                size: 12,
                hash,
                publication_id: PublicationId(id),
            }],
            8,
        )
        .unwrap();
        let expected = vectors.bytes("V2", "content");
        assert_eq!(manifest.encode(), expected);
        assert_eq!(expected.len(), 131);
        assert_eq!(Manifest::decode(&expected, 8).unwrap(), manifest);
        assert!(manifest.entry(&CanonicalPath::root()).is_some());
        assert!(
            manifest
                .entry(&CanonicalPath::parse(b"/x").unwrap())
                .is_none()
        );
    }

    // Entries are sorted by path bytes on build, so insertion order never changes the canonical bytes, and several entries round trip.
    #[test]
    fn entries_are_canonically_ordered() {
        let a = Manifest::new(
            vec![
                entry("/b", "text/css", 3),
                entry("/", "text/html", 1),
                entry("/a/x", "image/png", 2),
            ],
            8,
        )
        .unwrap();
        let b = Manifest::new(
            vec![
                entry("/a/x", "image/png", 2),
                entry("/b", "text/css", 3),
                entry("/", "text/html", 1),
            ],
            8,
        )
        .unwrap();
        assert_eq!(a.encode(), b.encode());
        let paths: Vec<_> = a
            .entries()
            .iter()
            .map(|e| e.path.as_str().to_string())
            .collect();
        assert_eq!(paths, ["/", "/a/x", "/b"]);
        assert_eq!(Manifest::decode(&a.encode(), 8).unwrap(), a);
    }

    // HP-08: a missing root, a root that is not HTML, duplicate paths, no entries and too many entries are refused when building.
    #[test]
    fn building_enforces_the_page_rules() {
        let root = entry("/", "text/html", 1);
        assert!(Manifest::new(vec![root.clone()], 1).is_ok());
        assert_eq!(Manifest::new(vec![], 8), Err(Error::Invalid));
        assert_eq!(
            Manifest::new(vec![entry("/a", "text/html", 2)], 8),
            Err(Error::Invalid)
        );
        assert_eq!(
            Manifest::new(vec![entry("/", "text/plain", 1)], 8),
            Err(Error::Invalid)
        );
        assert_eq!(
            Manifest::new(vec![root.clone(), entry("/", "text/html", 2)], 8),
            Err(Error::Invalid)
        );
        assert_eq!(
            Manifest::new(vec![root.clone(), entry("/a", "text/css", 2)], 1),
            Err(Error::Invalid)
        );
        let long = format!(
            "/{}/{}/{}/{}/x",
            "a".repeat(255),
            "b".repeat(255),
            "c".repeat(255),
            "d".repeat(255)
        );
        assert!(long.len() > MAXIMUM_PATH_BYTES);
        assert_eq!(
            Manifest::new(vec![root, entry(&long, "text/css", 2)], 8),
            Err(Error::Invalid)
        );
    }

    // HP-08: decoding refuses a wrong domain, a zero or excessive count, truncation at every length, trailing bytes, unsorted or duplicated
    // entries, a bad path or media type, an oversized string length and a count the bytes could not hold, without allocating for them.
    #[test]
    fn decoding_is_strict_and_bounded() {
        let good = Manifest::new(
            vec![entry("/", "text/html", 1), entry("/a", "text/css", 2)],
            8,
        )
        .unwrap()
        .encode();
        assert!(Manifest::decode(&good, 8).is_ok());
        assert_eq!(Manifest::decode(&good, 1), Err(Error::Invalid));
        for cut in 0..good.len() {
            assert_eq!(
                Manifest::decode(&good[..cut], 8),
                Err(Error::Invalid),
                "cut {cut}"
            );
        }
        let mut trailing = good.clone();
        trailing.push(0);
        assert_eq!(Manifest::decode(&trailing, 8), Err(Error::Invalid));
        let mut wrong_domain = good.clone();
        wrong_domain[0] ^= 1;
        assert_eq!(Manifest::decode(&wrong_domain, 8), Err(Error::Invalid));
        let at = DOMAIN.len();
        for count in [0u32, 3, u32::MAX] {
            let mut counted = good.clone();
            counted[at..at + 4].copy_from_slice(&count.to_be_bytes());
            assert_eq!(
                Manifest::decode(&counted, 8),
                Err(Error::Invalid),
                "count {count}"
            );
        }
        let mut huge_path = good.clone();
        huge_path[at + 4..at + 12].copy_from_slice(&u64::MAX.to_be_bytes());
        assert_eq!(Manifest::decode(&huge_path, 8), Err(Error::Invalid));
        // Swapping the two entries' order or repeating one breaks the canonical order.
        let single = |e: &Entry| {
            Manifest {
                entries: vec![e.clone()],
            }
            .encode()
        };
        let (first, second) = (entry("/", "text/html", 1), entry("/a", "text/css", 2));
        let body = |bytes: Vec<u8>| bytes[at + 4..].to_vec();
        let mut swapped = Vec::from(DOMAIN);
        swapped.extend_from_slice(&2u32.to_be_bytes());
        swapped.extend(body(single(&second)));
        swapped.extend(body(single(&first)));
        assert_eq!(Manifest::decode(&swapped, 8), Err(Error::Invalid));
        let mut repeated = Vec::from(DOMAIN);
        repeated.extend_from_slice(&2u32.to_be_bytes());
        repeated.extend(body(single(&first)));
        repeated.extend(body(single(&first)));
        assert_eq!(Manifest::decode(&repeated, 8), Err(Error::Invalid));
        // An invalid path inside an otherwise well-formed entry.
        let bad_path = good.windows(1).position(|w| w[0] == b'a').unwrap();
        let mut backslash = good.clone();
        backslash[bad_path] = b'\\';
        assert_eq!(Manifest::decode(&backslash, 8), Err(Error::Invalid));
        let media_at = good.windows(4).position(|w| w == b"text").unwrap();
        let mut uppercase = good.clone();
        uppercase[media_at] = b'T';
        assert_eq!(Manifest::decode(&uppercase, 8), Err(Error::Invalid));
    }
}
