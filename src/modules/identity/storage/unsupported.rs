//! Explicit unavailable platform adapter; no fallback writes unprotected keys.
use super::super::{
    authority::{Administration, Capability},
    config::Limits,
    error::IdentityError,
};
use std::path::Path;
/// Placeholder type whose constructor rejects platforms without a protected adapter.
pub struct FileStore;
impl FileStore {
    /// Refuse key-store startup until this platform has a tested protection adapter.
    /// # Errors
    /// Always returns Unsupported and performs no filesystem effects.
    pub fn open(
        _path: &Path,
        _limits: Limits,
    ) -> Result<(Self, Capability<Administration>), IdentityError> {
        tracing::trace!("Rejecting unavailable protected platform storage");
        Err(IdentityError::Unsupported)
    }
}
