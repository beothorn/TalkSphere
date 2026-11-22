//! Authenticated encryption of hosted data before it leaves the node as a replica.
//!
//! Callers: the replica book seals each item once and every replica holds only the sealed bytes, so a
//! client that stores a replica learns nothing about the data. The 32-byte data key lives in an
//! owner-only file inside the node root; it must be backed up with the node's identity because no
//! replica can be opened without it. Each sealed unit is `version || nonce || ciphertext || tag`,
//! XChaCha20-Poly1305 with a fresh random 24-byte nonce, and the item identifier is authenticated as
//! associated data so a replica cannot be passed off as a different item.
use crate::modules::cryptography::{
    primitives::aead,
    secrets::{Content, Secret},
};
use std::{
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};
use zeroize::Zeroizing;

/// Format version byte of a sealed unit.
pub const VERSION: u8 = 1;
/// Bytes added to the plaintext: version, nonce and tag.
pub const OVERHEAD: usize = 1 + NONCE_BYTES + TAG_BYTES;
const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;
const KEY_BYTES: usize = 32;
/// Largest sealed unit handled, matching the finite object profile with room for the overhead.
const MAXIMUM_BYTES: usize = 65536;

/// Why sealing or opening failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealError {
    /// The key file is missing its protection, has the wrong size or could not be written.
    Key,
    /// No entropy was available for a nonce.
    Entropy,
    /// The input is too large or too short.
    Size,
    /// The unit did not authenticate: wrong key, wrong item, wrong version or tampered bytes.
    Authentication,
}

/// The node's replica data key.
pub struct Sealer {
    key: Secret<Content>,
}

impl Sealer {
    /// Read the key file at `path`, creating a random key with owner-only permissions when it is absent.
    /// # Errors
    /// `Key` when an existing file is not exactly 32 bytes or is readable by anyone but its owner, or
    /// when a new key cannot be written; an existing key is never replaced.
    pub fn open_or_create(path: &Path) -> Result<Self, SealError> {
        tracing::trace!("Opening the replica data key");
        match std::fs::read(path) {
            Ok(bytes) => {
                let mode = std::fs::metadata(path)
                    .map_err(|_| SealError::Key)?
                    .permissions()
                    .mode();
                if mode & 0o077 != 0 {
                    return Err(SealError::Key);
                }
                Self::from_bytes(&bytes)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut key = Zeroizing::new([0u8; KEY_BYTES]);
                getrandom::fill(key.as_mut_slice()).map_err(|_| SealError::Entropy)?;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(path)
                    .map_err(|_| SealError::Key)?;
                file.write_all(key.as_slice())
                    .and_then(|()| file.sync_all())
                    .map_err(|_| SealError::Key)?;
                Ok(Self {
                    key: Secret::new(key),
                })
            }
            Err(_) => Err(SealError::Key),
        }
    }

    // A key from exactly 32 bytes.
    fn from_bytes(bytes: &[u8]) -> Result<Self, SealError> {
        let array: [u8; KEY_BYTES] = bytes.try_into().map_err(|_| SealError::Key)?;
        Ok(Self {
            key: Secret::new(Zeroizing::new(array)),
        })
    }

    /// Seal `plaintext`, binding it to `context` (the item identifier).
    /// # Errors
    /// `Size` for empty or oversized input, `Entropy` when no nonce could be drawn.
    pub fn seal(&self, plaintext: &[u8], context: &[u8]) -> Result<Vec<u8>, SealError> {
        tracing::trace!("Sealing hosted data for replication");
        if plaintext.is_empty() || plaintext.len() + OVERHEAD > MAXIMUM_BYTES {
            return Err(SealError::Size);
        }
        let mut nonce = [0u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| SealError::Entropy)?;
        let sealed = aead::seal(&self.key, &nonce, plaintext, &aad(context), MAXIMUM_BYTES)
            .map_err(|_| SealError::Size)?;
        let mut unit = Vec::with_capacity(OVERHEAD + plaintext.len());
        unit.push(VERSION);
        unit.extend_from_slice(&nonce);
        unit.extend_from_slice(&sealed);
        Ok(unit)
    }

    /// Authenticate and open a sealed unit for `context`; no plaintext is released on failure.
    /// # Errors
    /// `Size` for a unit that cannot hold a version, nonce and tag, `Authentication` for anything that
    /// does not authenticate under this key and context.
    pub fn open(&self, unit: &[u8], context: &[u8]) -> Result<Zeroizing<Vec<u8>>, SealError> {
        tracing::trace!("Opening a replica");
        if unit.len() < OVERHEAD || unit.len() > MAXIMUM_BYTES {
            return Err(SealError::Size);
        }
        let (version, rest) = unit.split_first().ok_or(SealError::Size)?;
        if *version != VERSION {
            return Err(SealError::Authentication);
        }
        let (nonce, sealed) = rest.split_at(NONCE_BYTES);
        aead::open(&self.key, nonce, sealed, &aad(context), MAXIMUM_BYTES)
            .map(Zeroizing::new)
            .map_err(|_| SealError::Authentication)
    }
}

// Associated data: a fixed domain label then the context.
fn aad(context: &[u8]) -> Vec<u8> {
    let mut data = b"talksphere.replica.v1:".to_vec();
    data.extend_from_slice(context);
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    // A fresh private directory per test.
    fn dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "ts-sealer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    // Sealing then opening returns the plaintext, the unit has the documented overhead and never shows
    // the plaintext, and two seals of the same data differ because the nonce is fresh.
    #[test]
    fn seal_and_open_round_trip() {
        let sealer = Sealer::open_or_create(&dir().join("key")).unwrap();
        let unit = sealer.seal(b"hosted message", b"item-1").unwrap();
        assert_eq!(unit.len(), b"hosted message".len() + OVERHEAD);
        assert_eq!(unit[0], VERSION);
        assert!(!unit.windows(6).any(|w| w == b"hosted"));
        assert_eq!(&*sealer.open(&unit, b"item-1").unwrap(), b"hosted message");
        assert_ne!(unit, sealer.seal(b"hosted message", b"item-1").unwrap());
    }

    // Wrong item, wrong key, wrong version, tampering and truncation all fail without plaintext.
    #[test]
    fn anything_that_does_not_authenticate_is_refused() {
        let root = dir();
        let sealer = Sealer::open_or_create(&root.join("key")).unwrap();
        let other = Sealer::open_or_create(&root.join("other")).unwrap();
        let unit = sealer.seal(b"secret", b"a").unwrap();
        assert_eq!(
            sealer.open(&unit, b"b").err(),
            Some(SealError::Authentication)
        );
        assert_eq!(
            other.open(&unit, b"a").err(),
            Some(SealError::Authentication)
        );
        let mut bad = unit.clone();
        bad[0] = 2;
        assert_eq!(
            sealer.open(&bad, b"a").err(),
            Some(SealError::Authentication)
        );
        for index in [1, 30, unit.len() - 1] {
            let mut tampered = unit.clone();
            tampered[index] ^= 1;
            assert_eq!(
                sealer.open(&tampered, b"a").err(),
                Some(SealError::Authentication)
            );
        }
        assert_eq!(
            sealer.open(&unit[..OVERHEAD - 1], b"a").err(),
            Some(SealError::Size)
        );
        assert_eq!(sealer.open(&[], b"a").err(), Some(SealError::Size));
    }

    // Empty and oversized plaintext are refused before any encryption.
    #[test]
    fn sizes_are_bounded() {
        let sealer = Sealer::open_or_create(&dir().join("key")).unwrap();
        assert_eq!(sealer.seal(b"", b"a").err(), Some(SealError::Size));
        assert_eq!(
            sealer.seal(&vec![0; MAXIMUM_BYTES], b"a").err(),
            Some(SealError::Size)
        );
        assert!(
            sealer
                .seal(&vec![0; MAXIMUM_BYTES - OVERHEAD], b"a")
                .is_ok()
        );
    }

    // The key persists across opens, is owner-only, and a bad file is refused without being replaced.
    #[test]
    fn the_key_file_is_stable_private_and_never_replaced() {
        let root = dir();
        let path = root.join("key");
        let unit = Sealer::open_or_create(&path)
            .unwrap()
            .seal(b"x", b"a")
            .unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(
            Sealer::open_or_create(&path)
                .unwrap()
                .open(&unit, b"a")
                .is_ok()
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(Sealer::open_or_create(&path).err(), Some(SealError::Key));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(&path, b"short").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(Sealer::open_or_create(&path).err(), Some(SealError::Key));
        assert_eq!(std::fs::read(&path).unwrap(), b"short");
    }
}
