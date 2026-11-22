//! Owner-only native file helpers shared by the placement store and the participant journal.
//!
//! Callers: `store` and `journal`. Directories are created 0700 and files 0600, and every write goes to a private
//! temporary file that is flushed before being renamed over its target, so a crash leaves either the old or the new file.
use super::error::Error;
use std::{
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
};

/// Create a directory (and parents) readable only by its owner.
/// # Errors
/// `Storage` when it cannot be created.
pub(super) fn create_dir(dir: &Path) -> Result<(), Error> {
    tracing::trace!("Creating an owner-only Messaging directory");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|_| Error::Storage)
}

/// Replace `path` atomically with `bytes`: write a private temporary file, flush it, then rename it over the target.
/// # Errors
/// `Storage` on any failure; the previous file is left in place.
pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    tracing::trace!("Writing a Messaging file atomically");
    let temporary = path.with_extension("tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| Error::Storage)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| Error::Storage)?;
    std::fs::rename(&temporary, path).map_err(|_| Error::Storage)
}

/// Lowercase hexadecimal text of bytes.
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Decode strict lowercase hexadecimal into exactly N bytes.
pub(super) fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Decode strict lowercase hexadecimal of any length.
pub(super) fn unhex_vec(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2)
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}
