//! The `cr2se.identity-encrypted.v1` payload: encryption to a recipient's persistent X25519 key.
//!
//! Callers: a sender encrypts an application message before storing it; the recipient decrypts after
//! verifying the sender signature and payload hash. The provider never encrypts or decrypts. The stored
//! payload is `0x01 || recipientKey(32) || ephemeralKey(32) || nonce(24) || ciphertext+tag`, at least 105
//! bytes. The key is HKDF-SHA-256 over the X25519 shared secret with the SHA-256 of the encryption metadata
//! bytes as salt and `CR2SE-MESSAGE-PAYLOAD-KEY-V1` as info; XChaCha20-Poly1305 authenticates the metadata
//! bytes, binding the ciphertext to the message, sender, recipient and creation time. A failed decryption
//! yields no plaintext. This is not the generic identity envelope and must never be substituted for it.
use super::{error::Error, metadata::Metadata, ports::Entropy, state::MINIMUM_ENCRYPTED_BYTES};
use crate::modules::cryptography::{
    api::KeyPair,
    primitives::{aead, agreement, hash, kdf},
    secrets::{Content, Ephemeral, Persistent, Secret},
};
use zeroize::Zeroizing;

/// Format version byte.
pub const VERSION: u8 = 1;
/// Domain string of the encryption metadata bytes.
pub const METADATA_DOMAIN: &[u8] = b"CR2SE-MESSAGE-ENCRYPTION-METADATA-V1";
/// HKDF info string.
pub const KEY_INFO: &[u8] = b"CR2SE-MESSAGE-PAYLOAD-KEY-V1";
const NONCE_BYTES: usize = 24;
const MAXIMUM_BYTES: usize = 1 << 17;

/// The encryption metadata bytes: `METADATA_DOMAIN || messageId || senderId || recipientId || u64be(senderCreatedAt)`.
/// They are not secret; they bind the ciphertext to the visible message metadata.
pub fn metadata_bytes(metadata: &Metadata) -> Vec<u8> {
    tracing::trace!("Building the encryption metadata bytes");
    let mut bytes = Vec::with_capacity(METADATA_DOMAIN.len() + 32 * 3 + 8);
    bytes.extend_from_slice(METADATA_DOMAIN);
    bytes.extend_from_slice(&metadata.message_id.0);
    bytes.extend_from_slice(&metadata.sender_id.0);
    bytes.extend_from_slice(&metadata.recipient_id.0);
    bytes.extend_from_slice(&metadata.sender_created_at.to_be_bytes());
    bytes
}

// The message key from the shared secret and the exact metadata bytes.
fn message_key(
    shared: Secret<crate::modules::cryptography::secrets::Shared>,
    metadata: &[u8],
) -> Result<Secret<Content>, Error> {
    let salt = hash::sha256(metadata, metadata.len()).map_err(|_| Error::Invalid)?;
    let prk = kdf::extract(&salt, shared, MAXIMUM_BYTES).map_err(|_| Error::Invalid)?;
    kdf::expand_key::<Content>(&prk, KEY_INFO, MAXIMUM_BYTES).map_err(|_| Error::Invalid)
}

/// Encrypt with an explicit ephemeral key pair and nonce. This is the whole construction; production callers use
/// [`encrypt`], which draws both from secure entropy, and tests supply fixed values to reproduce independent vectors.
/// The ephemeral key is consumed. `recipient_key` must already be a verified 32-byte X25519 public key.
/// # Errors
/// `Invalid` for an empty plaintext, an oversized value or a low-order recipient key (an all-zero shared result).
pub fn encrypt_with(
    plaintext: &[u8],
    metadata: &Metadata,
    recipient_key: &[u8; 32],
    ephemeral: KeyPair<Secret<Ephemeral>>,
    nonce: [u8; NONCE_BYTES],
) -> Result<Vec<u8>, Error> {
    tracing::trace!("Encrypting a message payload to a recipient key");
    if plaintext.is_empty() || plaintext.len() + MINIMUM_ENCRYPTED_BYTES > MAXIMUM_BYTES {
        return Err(Error::Invalid);
    }
    let ephemeral_public = ephemeral.public_key;
    let shared =
        agreement::ephemeral(ephemeral.private_key, recipient_key).map_err(|_| Error::Invalid)?;
    let aad = metadata_bytes(metadata);
    let key = message_key(shared, &aad)?;
    let sealed =
        aead::seal(&key, &nonce, plaintext, &aad, MAXIMUM_BYTES).map_err(|_| Error::Invalid)?;
    let mut payload = Vec::with_capacity(1 + 32 + 32 + NONCE_BYTES + sealed.len());
    payload.push(VERSION);
    payload.extend_from_slice(recipient_key);
    payload.extend_from_slice(&ephemeral_public);
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&sealed);
    Ok(payload)
}

/// Encrypt `plaintext` to a verified recipient X25519 key with a fresh ephemeral key and nonce from `entropy`.
/// # Errors
/// `Unavailable` when randomness is missing (never a fallback); otherwise as [`encrypt_with`].
pub fn encrypt(
    plaintext: &[u8],
    metadata: &Metadata,
    recipient_key: &[u8; 32],
    entropy: &dyn Entropy,
) -> Result<Vec<u8>, Error> {
    tracing::trace!("Encrypting with fresh ephemeral material");
    let mut seed = Zeroizing::new([0u8; 32]);
    entropy.fill(seed.as_mut_slice())?;
    let public = x25519_public(&seed);
    let mut nonce = [0u8; NONCE_BYTES];
    entropy.fill(&mut nonce)?;
    encrypt_with(
        plaintext,
        metadata,
        recipient_key,
        KeyPair {
            public_key: public,
            private_key: Secret::new(seed),
        },
        nonce,
    )
}

// The X25519 public key of a private seed.
fn x25519_public(seed: &[u8; 32]) -> [u8; 32] {
    x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(*seed)).to_bytes()
}

/// Decrypt a stored payload as the recipient, after the caller verified the sender signature and payload hash.
/// Checks the version, that the embedded recipient key is `own_public_key` (the key whose private half is held), then derives the
/// message key and authenticates the ciphertext against the reconstructed metadata bytes.
/// # Errors
/// `Invalid` for a short payload, another version, a different recipient key, a low-order ephemeral key, or any
/// authentication failure (wrong key, changed metadata, tampered tag or ciphertext). No plaintext is returned on failure.
pub fn decrypt(
    payload: &[u8],
    metadata: &Metadata,
    own_key: &Secret<Persistent>,
    own_public_key: &[u8; 32],
) -> Result<Zeroizing<Vec<u8>>, Error> {
    tracing::trace!("Decrypting a recovered message payload");
    if payload.len() < MINIMUM_ENCRYPTED_BYTES || payload[0] != VERSION {
        return Err(Error::Invalid);
    }
    if payload[1..33] != own_public_key[..] {
        return Err(Error::Invalid);
    }
    let ephemeral_public = &payload[33..65];
    let nonce = &payload[65..65 + NONCE_BYTES];
    let sealed = &payload[65 + NONCE_BYTES..];
    let shared = agreement::persistent(own_key, ephemeral_public).map_err(|_| Error::Invalid)?;
    let aad = metadata_bytes(metadata);
    let key = message_key(shared, &aad)?;
    aead::open(&key, nonce, sealed, &aad, MAXIMUM_BYTES)
        .map(Zeroizing::new)
        .map_err(|_| Error::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::identity::api::IdentityId;
    use crate::modules::messaging::{
        ids::{MessageId, PayloadHash},
        state::Encoding,
        test_support::SequenceEntropy,
    };

    // The independent Messaging payload example from the Cryptography fixture (generated with PyNaCl, not TalkSphere):
    // message ID 00..1f, sender and recipient IDs and time 1000, recipient private key 0x60.., ephemeral private key 0x40..,
    // nonce 0x80.., plaintext "CR2SE identity encryption test".
    const FIXTURE: &str = include_str!("../cryptography/test_support/fixtures/messaging.md");

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }
    // The hex block that follows a label line in the fixture.
    fn block(label: &str) -> Vec<u8> {
        let start = FIXTURE.find(&format!("{label}:\n")).unwrap() + label.len() + 2;
        hex(FIXTURE[start..].lines().next().unwrap())
    }
    fn fixture_metadata() -> Metadata {
        // The fixture's metadata block begins with the domain string, then the three identifiers and the time.
        let m = block("metadata");
        let take = |at: usize| -> [u8; 32] { m[at..at + 32].try_into().unwrap() };
        let at = METADATA_DOMAIN.len();
        Metadata {
            message_id: MessageId(take(at)),
            sender_id: IdentityId(take(at + 32)),
            recipient_id: IdentityId(take(at + 64)),
            sender_created_at: u64::from_be_bytes(m[at + 96..at + 104].try_into().unwrap()),
            encoding: Encoding::IdentityEncryptedV1,
            payload_size: 0,
            payload_hash: PayloadHash([0; 32]),
        }
    }
    fn private(start: u8) -> [u8; 32] {
        std::array::from_fn(|i| start + i as u8)
    }
    fn persistent(seed: [u8; 32]) -> (Secret<Persistent>, [u8; 32]) {
        (Secret::new(Zeroizing::new(seed)), x25519_public(&seed))
    }

    // MS-14: encrypting the fixture inputs reproduces the independent payload byte for byte, its layout is as specified,
    // and the recipient decrypts it to the exact plaintext.
    #[test]
    fn ms_14_matches_the_independent_vector() {
        let metadata = fixture_metadata();
        assert_eq!(metadata_bytes(&metadata), block("metadata"));
        let (recipient_secret, recipient_public) = persistent(private(0x60));
        let ephemeral_seed = private(0x40);
        let nonce: [u8; 24] = block("nonce").try_into().unwrap();
        let payload = encrypt_with(
            b"CR2SE identity encryption test",
            &metadata,
            &recipient_public,
            KeyPair {
                public_key: x25519_public(&ephemeral_seed),
                private_key: Secret::new(Zeroizing::new(ephemeral_seed)),
            },
            nonce,
        )
        .unwrap();
        assert_eq!(payload, block("payload"));
        assert_eq!(payload[0], 1);
        assert_eq!(payload.len(), 105 + b"CR2SE identity encryption test".len());
        assert_eq!(&payload[1..33], &recipient_public);
        assert_eq!(
            &*decrypt(&payload, &metadata, &recipient_secret, &recipient_public).unwrap(),
            b"CR2SE identity encryption test"
        );
    }

    // The payload is 105 bytes plus the plaintext length, and fresh randomness gives a different ciphertext each time.
    #[test]
    fn length_and_freshness() {
        let metadata = fixture_metadata();
        let (secret, public) = persistent(private(0x60));
        let entropy = SequenceEntropy::new(1);
        let one = encrypt(b"x", &metadata, &public, &entropy).unwrap();
        let two = encrypt(b"x", &metadata, &public, &entropy).unwrap();
        assert_eq!(one.len(), 106);
        assert_ne!(one, two);
        assert_eq!(&*decrypt(&two, &metadata, &secret, &public).unwrap(), b"x");
    }

    // MS-14: a wrong recipient key, changed bound metadata, a tampered tag or ciphertext, another version and truncation
    // all yield no plaintext.
    #[test]
    fn every_failure_yields_no_plaintext() {
        let metadata = fixture_metadata();
        let (secret, public) = persistent(private(0x60));
        let payload = encrypt(b"secret", &metadata, &public, &SequenceEntropy::new(9)).unwrap();
        assert!(decrypt(&payload, &metadata, &secret, &public).is_ok());
        let (other_secret, other_public) = persistent(private(0x10));
        assert_eq!(
            decrypt(&payload, &metadata, &other_secret, &other_public).err(),
            Some(Error::Invalid)
        );
        // The right private key presented with the wrong public key is also refused.
        assert_eq!(
            decrypt(&payload, &metadata, &secret, &other_public).err(),
            Some(Error::Invalid)
        );
        let changed: [fn(&mut Metadata); 4] = [
            |m| m.message_id.0[0] ^= 1,
            |m| m.sender_id.0[0] ^= 1,
            |m| m.recipient_id.0[0] ^= 1,
            |m| m.sender_created_at += 1,
        ];
        for edit in changed {
            let mut m = metadata.clone();
            edit(&mut m);
            assert_eq!(
                decrypt(&payload, &m, &secret, &public).err(),
                Some(Error::Invalid)
            );
        }
        for index in [payload.len() - 1, payload.len() - 17, 40, 70] {
            let mut bad = payload.clone();
            bad[index] ^= 1;
            assert_eq!(
                decrypt(&bad, &metadata, &secret, &public).err(),
                Some(Error::Invalid),
                "{index}"
            );
        }
        let mut version = payload.clone();
        version[0] = 2;
        assert_eq!(
            decrypt(&version, &metadata, &secret, &public).err(),
            Some(Error::Invalid)
        );
        for length in [0, 1, 104, 105] {
            assert_eq!(
                decrypt(&payload[..length], &metadata, &secret, &public).err(),
                Some(Error::Invalid),
                "{length}"
            );
        }
    }

    // Encryption refuses an empty plaintext, an all-zero (low-order) recipient key and missing entropy, and never falls back.
    #[test]
    fn encryption_refusals() {
        let metadata = fixture_metadata();
        let (_, public) = persistent(private(0x60));
        let entropy = SequenceEntropy::new(1);
        assert_eq!(
            encrypt(b"", &metadata, &public, &entropy).err(),
            Some(Error::Invalid)
        );
        assert_eq!(
            encrypt(b"x", &metadata, &[0; 32], &entropy).err(),
            Some(Error::Invalid)
        );
        let broken = SequenceEntropy::new(1);
        broken.fail();
        assert_eq!(
            encrypt(b"x", &metadata, &public, &broken).err(),
            Some(Error::Unavailable)
        );
    }
}
