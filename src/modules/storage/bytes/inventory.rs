//! Bounded descriptor-anchored inventory exposes only owner object IDs and lengths for recovery accounting.
use super::{Store, fs_support};
use crate::modules::services::Error;
use rustix::fs::{self, Mode, OFlags};
use std::fs::File;

/// Protected local byte entry, never a remote lease authorization or filesystem path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Opaque owner object identity.
    pub id: [u8; 32],
    /// Actual local entry byte length.
    pub length: u64,
    /// True for provisional upload bytes that were never installed as a complete object.
    pub pending: bool,
}
impl Store {
    /// List only bounded protected object/upload entries through the retained directory descriptor.
    /// Fixed metadata/lock names are excluded; unknown entries fail closed rather than escaping accounting.
    /// # Errors
    /// Limit rejects excessive entry counts, Invalid noncanonical names, and Storage/Denied unsafe or unavailable files.
    pub fn inventory(&self, maximum_entries: usize) -> Result<Vec<Entry>, Error> {
        tracing::trace!("Inventorying bounded owner Storage bytes before admission");
        let directory = fs::Dir::read_from(&self.0.file).map_err(|_| Error::Storage)?;
        let mut entries = Vec::new();
        for entry in directory {
            let entry = entry.map_err(|_| Error::Storage)?;
            let name = entry.file_name().to_bytes();
            if matches!(
                name,
                b"." | b".." | b"writer.lock" | b"metadata.bin" | b"metadata.pending"
            ) {
                continue;
            }
            if entries.len() >= maximum_entries {
                return Err(Error::Limit);
            }
            let (pending, hex) = if let Some(h) = name.strip_prefix(b"upload-") {
                (true, h)
            } else if let Some(h) = name.strip_prefix(b"object-") {
                (false, h)
            } else {
                return Err(Error::Invalid);
            };
            if hex.len() != 64 {
                return Err(Error::Invalid);
            }
            let mut id = [0; 32];
            for (byte, pair) in id.iter_mut().zip(hex.as_chunks::<2>().0) {
                *byte = nibble(pair[0])? * 16 + nibble(pair[1])?;
            }
            let file = File::from(
                fs::openat(
                    &self.0.file,
                    super::name(id, pending),
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                    Mode::empty(),
                )
                .map_err(|_| Error::Storage)?,
            );
            fs_support::protected(&file, false)?;
            entries.push(Entry {
                id,
                length: file.metadata().map_err(|_| Error::Storage)?.len(),
                pending,
            });
        }
        Ok(entries)
    }
}
// Accept only the exact lowercase hex emitted by the local owner naming function.
fn nibble(byte: u8) -> Result<u8, Error> {
    tracing::trace!("Checking canonical Storage object inventory identifier");
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(Error::Invalid),
    }
}
