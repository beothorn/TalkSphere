//! The restricted-content envelope: one content key wrapped for each audience member.
//!
//! Callers: a publisher sealing restricted content before signing it, hosts checking its structure on store, and a reader opening
//! it after verifying the publication. The plaintext is encrypted once with a random content key; that key is wrapped for each reader
//! by X25519 with one ephemeral key, HKDF-SHA-256 (salt: SHA-256 of the encryption metadata bytes) and XChaCha20-Poly1305 that
//! authenticates the metadata and the reader's key. The stored layout is `0x01 || ephemeralPublicKey(32) || contentNonce(24) ||
//! u32be(entries) || entries (readerKey(32) || wrapNonce(24) || wrappedKey(48)) each || contentCiphertext(>=16)`. A host never
//! decrypts; a failed opening yields no plaintext.
use super::{error::Error, metadata::Metadata, signing::MAXIMUM_CONTENT_BYTES};
use crate::modules::{
    cryptography::{
        primitives::{aead, agreement, hash, kdf},
        secrets::{Content, Ephemeral, Secret},
    },
    messaging::{encryption::PersistentAgreement, ports::Entropy},
};
use zeroize::Zeroizing;

/// Envelope format version byte.
pub const VERSION: u8 = 1;
/// Domain string of the encryption metadata bytes.
pub const METADATA_DOMAIN: &[u8] = b"CR2SE-PUBLICATION-ENCRYPTION-METADATA-V1";
/// HKDF info string of the key wrap.
pub const KEY_INFO: &[u8] = b"CR2SE-PUBLICATION-KEY-WRAP-V1";
/// Bytes before the entries: version, ephemeral key, content nonce and entry count.
pub const HEADER_BYTES: usize = 1 + 32 + 24 + 4;
/// Bytes of one reader entry: reader key, wrap nonce and wrapped content key.
pub const ENTRY_BYTES: usize = 32 + 24 + 48;
/// Smallest content ciphertext: the authentication tag of an empty plaintext, which publications never carry.
pub const MINIMUM_CIPHERTEXT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const AEAD_LIMIT: usize = 1 << 17;

/// The encryption metadata bytes: `METADATA_DOMAIN || publicationId || publisherId || kindCode || u64be(sequence) ||
/// u64be(publishedAt)`. They are not secret; they bind the ciphertext and every wrapped key to the visible metadata.
pub fn metadata_bytes(metadata: &Metadata) -> Vec<u8> {
    tracing::trace!("Building the publication encryption metadata bytes");
    let mut bytes = Vec::with_capacity(METADATA_DOMAIN.len() + 32 * 2 + 17);
    bytes.extend_from_slice(METADATA_DOMAIN);
    bytes.extend_from_slice(&metadata.publication_id.0);
    bytes.extend_from_slice(&metadata.publisher_id.0);
    bytes.push(metadata.kind.code());
    bytes.extend_from_slice(&metadata.sequence.to_be_bytes());
    bytes.extend_from_slice(&metadata.published_at.to_be_bytes());
    bytes
}

/// Every random value of one sealing, so tests can reproduce the independent vector and production draws them from entropy.
pub struct Randomness {
    /// The ephemeral X25519 private key shared by all wraps.
    pub ephemeral_private: Zeroizing<[u8; 32]>,
    /// The content key.
    pub content_key: Zeroizing<[u8; 32]>,
    /// The nonce of the content ciphertext.
    pub content_nonce: [u8; NONCE_BYTES],
    /// One wrap nonce per reader, in audience order.
    pub wrap_nonces: Vec<[u8; NONCE_BYTES]>,
}

impl Randomness {
    /// Draw fresh values for `readers` readers from a secure source.
    /// # Errors
    /// `Unavailable` when randomness cannot be obtained; no value is ever derived from a fallback.
    pub fn draw(readers: usize, entropy: &dyn Entropy) -> Result<Self, Error> {
        tracing::trace!("Drawing envelope randomness");
        let mut ephemeral_private = Zeroizing::new([0u8; 32]);
        entropy.fill(ephemeral_private.as_mut_slice())?;
        let mut content_key = Zeroizing::new([0u8; 32]);
        entropy.fill(content_key.as_mut_slice())?;
        let mut content_nonce = [0u8; NONCE_BYTES];
        entropy.fill(&mut content_nonce)?;
        let mut wrap_nonces = Vec::with_capacity(readers);
        for _ in 0..readers {
            let mut nonce = [0u8; NONCE_BYTES];
            entropy.fill(&mut nonce)?;
            wrap_nonces.push(nonce);
        }
        Ok(Self {
            ephemeral_private,
            content_key,
            content_nonce,
            wrap_nonces,
        })
    }
}

/// Seal `plaintext` for `reader_keys`, which must be the verified X25519 keys of the audience in audience order.
/// # Errors
/// `Invalid` for empty or oversized plaintext, no readers or more than the finite audience, a wrong number of wrap nonces, or a
/// reader key that yields an invalid (all-zero) shared secret; `Unavailable` is never returned here.
pub fn seal_with(
    plaintext: &[u8],
    metadata: &Metadata,
    reader_keys: &[[u8; 32]],
    randomness: Randomness,
) -> Result<Vec<u8>, Error> {
    tracing::trace!("Sealing restricted content for its audience");
    // The whole envelope, not just the plaintext, must fit the content bound.
    let envelope_bytes = HEADER_BYTES
        .saturating_add(ENTRY_BYTES.saturating_mul(reader_keys.len()))
        .saturating_add(plaintext.len())
        .saturating_add(MINIMUM_CIPHERTEXT_BYTES);
    if plaintext.is_empty()
        || envelope_bytes > MAXIMUM_CONTENT_BYTES
        || reader_keys.is_empty()
        || reader_keys.len() != randomness.wrap_nonces.len()
    {
        return Err(Error::Invalid);
    }
    let aad = metadata_bytes(metadata);
    let salt = hash::sha256(&aad, aad.len()).map_err(|_| Error::Invalid)?;
    let ephemeral_public = x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(
        *randomness.ephemeral_private,
    ))
    .to_bytes();
    let content_key = Secret::<Content>::new(Zeroizing::new(*randomness.content_key));
    let ciphertext = aead::seal(
        &content_key,
        &randomness.content_nonce,
        plaintext,
        &aad,
        AEAD_LIMIT,
    )
    .map_err(|_| Error::Invalid)?;
    let mut sealed =
        Vec::with_capacity(HEADER_BYTES + ENTRY_BYTES * reader_keys.len() + ciphertext.len());
    sealed.push(VERSION);
    sealed.extend_from_slice(&ephemeral_public);
    sealed.extend_from_slice(&randomness.content_nonce);
    sealed.extend_from_slice(&(reader_keys.len() as u32).to_be_bytes());
    for (reader, nonce) in reader_keys.iter().zip(&randomness.wrap_nonces) {
        let shared = agreement::ephemeral(
            Secret::<Ephemeral>::new(randomness.ephemeral_private.clone()),
            reader,
        )
        .map_err(|_| Error::Invalid)?;
        let wrap_key = wrap_key(shared, &salt)?;
        let mut wrap_aad = aad.clone();
        wrap_aad.extend_from_slice(reader);
        let wrapped = aead::seal(
            &wrap_key,
            nonce,
            randomness.content_key.as_slice(),
            &wrap_aad,
            AEAD_LIMIT,
        )
        .map_err(|_| Error::Invalid)?;
        sealed.extend_from_slice(reader);
        sealed.extend_from_slice(nonce);
        sealed.extend_from_slice(&wrapped);
    }
    sealed.extend_from_slice(&ciphertext);
    Ok(sealed)
}

/// Seal `plaintext` with fresh randomness from `entropy`.
/// # Errors
/// `Unavailable` when randomness is missing; otherwise as [`seal_with`].
pub fn seal(
    plaintext: &[u8],
    metadata: &Metadata,
    reader_keys: &[[u8; 32]],
    entropy: &dyn Entropy,
) -> Result<Vec<u8>, Error> {
    tracing::trace!("Sealing restricted content with fresh randomness");
    seal_with(
        plaintext,
        metadata,
        reader_keys,
        Randomness::draw(reader_keys.len(), entropy)?,
    )
}

/// The smallest envelope for `entries` readers: the header, every entry and a content ciphertext tag. A restricted publication whose
/// declared size is below this cannot be valid, so a host can refuse it from the header alone.
pub fn minimum_bytes(entries: usize) -> usize {
    HEADER_BYTES
        .saturating_add(ENTRY_BYTES.saturating_mul(entries))
        .saturating_add(MINIMUM_CIPHERTEXT_BYTES)
}

/// Check the structure a host can verify without any key: the version byte, an entry count equal to the audience count and a
/// length of exactly the header, all entries and at least a tag.
/// # Errors
/// `Invalid` for any mismatch.
pub fn check_structure(content: &[u8], audience_count: usize) -> Result<(), Error> {
    tracing::trace!("Checking the restricted envelope structure");
    let entries = declared_entries(content)?;
    if entries != audience_count || entries == 0 {
        return Err(Error::Invalid);
    }
    if content.len() < minimum_bytes(entries) {
        return Err(Error::Invalid);
    }
    Ok(())
}

// The entry count a header declares, after checking the version byte and that the header is present.
fn declared_entries(content: &[u8]) -> Result<usize, Error> {
    if content.len() < HEADER_BYTES || content[0] != VERSION {
        return Err(Error::Invalid);
    }
    let count: [u8; 4] = content[57..61].try_into().map_err(|_| Error::Invalid)?;
    usize::try_from(u32::from_be_bytes(count)).map_err(|_| Error::Invalid)
}

/// Open an envelope as an audience member, after the caller verified the publication.
/// `own_public_key` is the X25519 public key whose private half `own_key` holds; the entry carrying it is unwrapped and the content
/// decrypted against the metadata bytes.
/// # Errors
/// `Invalid` for a bad structure or version, no entry for this key, a low-order ephemeral key, or any authentication failure (wrong
/// key, changed metadata, damaged wrap or ciphertext). No plaintext is returned on failure.
pub fn open(
    content: &[u8],
    metadata: &Metadata,
    own_key: &dyn PersistentAgreement,
    own_public_key: &[u8; 32],
) -> Result<Zeroizing<Vec<u8>>, Error> {
    tracing::trace!("Opening a restricted envelope as an audience member");
    let entries = declared_entries(content)?;
    check_structure(content, entries)?;
    let ephemeral: [u8; 32] = content[1..33].try_into().map_err(|_| Error::Invalid)?;
    let content_nonce = &content[33..57];
    let table = &content[HEADER_BYTES..HEADER_BYTES + entries * ENTRY_BYTES];
    let ciphertext = &content[HEADER_BYTES + entries * ENTRY_BYTES..];
    let entry = table
        .as_chunks::<ENTRY_BYTES>()
        .0
        .iter()
        .find(|entry| entry[..32] == own_public_key[..])
        .ok_or(Error::Invalid)?;
    let aad = metadata_bytes(metadata);
    let salt = hash::sha256(&aad, aad.len()).map_err(|_| Error::Invalid)?;
    let shared = own_key.agree(&ephemeral)?;
    let wrap_key = wrap_key(shared, &salt)?;
    let mut wrap_aad = aad.clone();
    wrap_aad.extend_from_slice(own_public_key);
    let wrapped = aead::open(
        &wrap_key,
        &entry[32..56],
        &entry[56..],
        &wrap_aad,
        AEAD_LIMIT,
    )
    .map_err(|_| Error::Invalid)?;
    let key_bytes: [u8; 32] = wrapped.try_into().map_err(|_| Error::Invalid)?;
    let content_key = Secret::<Content>::new(Zeroizing::new(key_bytes));
    aead::open(&content_key, content_nonce, ciphertext, &aad, AEAD_LIMIT)
        .map(Zeroizing::new)
        .map_err(|_| Error::Invalid)
}

// The key-wrap key from the shared secret and the salt: HKDF-SHA-256 with the module's info string.
fn wrap_key(
    shared: Secret<crate::modules::cryptography::secrets::Shared>,
    salt: &[u8; 32],
) -> Result<Secret<Content>, Error> {
    tracing::trace!("Deriving a key-wrap key");
    let prk = kdf::extract(salt, shared, AEAD_LIMIT).map_err(|_| Error::Invalid)?;
    kdf::expand_key::<Content>(&prk, KEY_INFO, AEAD_LIMIT).map_err(|_| Error::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{
        cryptography::secrets::Persistent,
        hosted_publications::{
            ids::{ContentHash, PublicationId},
            metadata::{Kind, Visibility},
            signing::tests::Vectors,
        },
        identity::api::IdentityId,
        messaging::test_support::SequenceEntropy,
    };

    // The vector's metadata: publication 40..5f, the vector publisher, feed post, sequence 7, time 1002.
    fn vector_metadata(vectors: &Vectors) -> Metadata {
        Metadata {
            publication_id: PublicationId(
                vectors
                    .bytes("V3 restricted feed post (envelope)", "publicationId")
                    .try_into()
                    .unwrap(),
            ),
            publisher_id: IdentityId(vectors.bytes("inputs", "publisherId").try_into().unwrap()),
            kind: Kind::FeedPost,
            sequence: 7,
            published_at: 1002,
            visibility: Visibility::Restricted,
            audience: vec![IdentityId(
                vectors
                    .bytes("inputs", "readerId (audience)")
                    .try_into()
                    .unwrap(),
            )],
            content_size: 295,
            content_hash: ContentHash([0; 32]),
        }
    }

    // The fixed inputs of the vector: private keys and nonces are the counting byte ranges the vector file states.
    fn run(start: u8) -> [u8; 32] {
        std::array::from_fn(|i| start + i as u8)
    }

    fn vector_randomness() -> Randomness {
        Randomness {
            ephemeral_private: Zeroizing::new(run(0x80)),
            content_key: Zeroizing::new(run(0xc0)),
            content_nonce: std::array::from_fn(|i| 0xe0 + i as u8),
            wrap_nonces: vec![std::array::from_fn(|i| 0xa0 + i as u8)],
        }
    }

    // HP-07: sealing the vector inputs reproduces the independent encryption metadata, envelope and ciphertext byte for byte, and
    // the reader with the vector key opens it to the feed message plaintext.
    #[test]
    fn the_vector_envelope_is_reproduced_and_opens() {
        let vectors = Vectors::load();
        let section = "V3 restricted feed post: encryption";
        let metadata = vector_metadata(&vectors);
        assert_eq!(
            metadata_bytes(&metadata),
            vectors.bytes(section, "encryptionMetadataBytes")
        );
        let plaintext = vectors.bytes(section, "feedMessageBytes (plaintext)");
        let reader_public = vectors.bytes("inputs", "readerEncryptionPublicKey");
        let sealed = seal_with(
            &plaintext,
            &metadata,
            &[reader_public.clone().try_into().unwrap()],
            vector_randomness(),
        )
        .unwrap();
        let expected = vectors.bytes("V3 restricted feed post (envelope)", "content");
        assert_eq!(sealed, expected);
        assert_eq!(sealed.len(), 295);
        assert_eq!(check_structure(&sealed, 1), Ok(()));
        let reader_private = Secret::<Persistent>::new(Zeroizing::new(run(0x40)));
        let opened = open(
            &sealed,
            &metadata,
            &reader_private,
            &reader_public.try_into().unwrap(),
        )
        .unwrap();
        assert_eq!(opened.as_slice(), plaintext.as_slice());
    }

    // Several readers each open the same plaintext with their own key; a stranger with another key finds no entry.
    #[test]
    fn every_audience_member_opens_and_strangers_do_not() {
        let vectors = Vectors::load();
        let metadata = vector_metadata(&vectors);
        let privates: Vec<Secret<Persistent>> = (1..=3u8)
            .map(|n| Secret::new(Zeroizing::new([n; 32])))
            .collect();
        let publics: Vec<[u8; 32]> = (1..=3u8)
            .map(|n| {
                x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from([n; 32])).to_bytes()
            })
            .collect();
        let sealed = seal(
            b"secret post",
            &metadata,
            &publics,
            &SequenceEntropy::new(1),
        )
        .unwrap();
        assert_eq!(sealed.len(), HEADER_BYTES + 3 * ENTRY_BYTES + 11 + 16);
        assert_eq!(check_structure(&sealed, 3), Ok(()));
        for (private, public) in privates.iter().zip(&publics) {
            let opened = open(&sealed, &metadata, private, public).unwrap();
            assert_eq!(opened.as_slice(), b"secret post");
        }
        let stranger = Secret::<Persistent>::new(Zeroizing::new([9; 32]));
        let stranger_public =
            x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from([9; 32])).to_bytes();
        assert_eq!(
            open(&sealed, &metadata, &stranger, &stranger_public),
            Err(Error::Invalid)
        );
        // The right public key with the wrong private key authenticates nothing either.
        assert_eq!(
            open(&sealed, &metadata, &stranger, &publics[0]),
            Err(Error::Invalid)
        );
    }

    // HP-07: a damaged wrapped key, damaged ciphertext, altered metadata, wrong version, count mismatch and truncation all fail.
    #[test]
    fn damaged_or_altered_envelopes_fail_without_plaintext() {
        let vectors = Vectors::load();
        let metadata = vector_metadata(&vectors);
        let reader_public: [u8; 32] = vectors
            .bytes("inputs", "readerEncryptionPublicKey")
            .try_into()
            .unwrap();
        let reader = Secret::<Persistent>::new(Zeroizing::new(run(0x40)));
        let good = vectors.bytes("V3 restricted feed post (envelope)", "content");
        let opens = |bytes: &[u8], m: &Metadata| open(bytes, m, &reader, &reader_public);
        assert!(opens(&good, &metadata).is_ok());
        let flip = |at: usize| {
            let mut bytes = good.clone();
            bytes[at] ^= 1;
            bytes
        };
        assert_eq!(
            opens(&flip(HEADER_BYTES + 60), &metadata),
            Err(Error::Invalid)
        );
        assert_eq!(opens(&flip(good.len() - 1), &metadata), Err(Error::Invalid));
        assert_eq!(opens(&flip(1), &metadata), Err(Error::Invalid));
        assert_eq!(opens(&flip(0), &metadata), Err(Error::Invalid));
        let mut altered = metadata.clone();
        altered.sequence += 1;
        assert_eq!(opens(&good, &altered), Err(Error::Invalid));
        assert_eq!(opens(&flip(60), &metadata), Err(Error::Invalid));
        assert_eq!(
            opens(&good[..good.len() - 1], &metadata),
            Err(Error::Invalid)
        );
        assert_eq!(opens(&good[..HEADER_BYTES], &metadata), Err(Error::Invalid));
        // A low-order ephemeral key (all zero) is refused by the agreement.
        let mut low_order = good.clone();
        low_order[1..33].fill(0);
        assert_eq!(opens(&low_order, &metadata), Err(Error::Invalid));
    }

    // The host structure check needs no key: it requires the version, an entry count equal to the audience and a full length.
    #[test]
    fn structure_checks_cover_version_count_and_length() {
        let vectors = Vectors::load();
        let good = vectors.bytes("V3 restricted feed post (envelope)", "content");
        assert_eq!(check_structure(&good, 1), Ok(()));
        assert_eq!(check_structure(&good, 2), Err(Error::Invalid));
        assert_eq!(check_structure(&good, 0), Err(Error::Invalid));
        let mut version = good.clone();
        version[0] = 2;
        assert_eq!(check_structure(&version, 1), Err(Error::Invalid));
        assert_eq!(
            check_structure(&good[..HEADER_BYTES + ENTRY_BYTES + 15], 1),
            Err(Error::Invalid)
        );
        assert_eq!(check_structure(&good[..10], 1), Err(Error::Invalid));
        // An entry count that would overflow the length arithmetic is just a mismatch.
        let mut huge = good.clone();
        huge[57..61].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(
            check_structure(&huge, u32::MAX as usize),
            Err(Error::Invalid)
        );
    }

    // Sealing refuses empty plaintext, no readers, a nonce count mismatch and a zero reader key; missing entropy is unavailable.
    #[test]
    fn sealing_refuses_bad_inputs() {
        let vectors = Vectors::load();
        let metadata = vector_metadata(&vectors);
        let key =
            x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from([4; 32])).to_bytes();
        let entropy = SequenceEntropy::new(3);
        assert_eq!(seal(b"", &metadata, &[key], &entropy), Err(Error::Invalid));
        assert_eq!(seal(b"x", &metadata, &[], &entropy), Err(Error::Invalid));
        assert_eq!(
            seal(b"x", &metadata, &[[0; 32]], &entropy),
            Err(Error::Invalid)
        );
        let mut mismatched = vector_randomness();
        mismatched.wrap_nonces.clear();
        assert_eq!(
            seal_with(b"x", &metadata, &[key], mismatched),
            Err(Error::Invalid)
        );
        entropy.fail();
        assert_eq!(
            seal(b"x", &metadata, &[key], &entropy),
            Err(Error::Unavailable)
        );
    }
}
