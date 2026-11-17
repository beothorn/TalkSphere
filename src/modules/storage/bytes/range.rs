//! Scoped immutable range references expose only accepted bytes while retaining the underlying descriptor.
use super::Handle;
use crate::modules::services::Error;

/// Immutable owner-issued subrange; callers cannot read outside its accepted length.
#[derive(Clone)]
pub struct Range {
    handle: Handle,
    offset: u64,
    length: u64,
}
impl Handle {
    /// Pin a positive exact subrange without allocating/copying content or exposing the full-object handle.
    /// # Errors
    /// Invalid rejects zero/out-of-object ranges using subtraction before addition.
    pub fn range(&self, offset: u64, length: u64) -> Result<Range, Error> {
        tracing::trace!("Pinning a scoped immutable Storage byte range");
        if length == 0 || offset >= self.length || length > self.length - offset {
            return Err(Error::Invalid);
        }
        Ok(Range {
            handle: self.clone(),
            offset,
            length,
        })
    }
}
impl Range {
    /// Return the accepted range length, in bytes, independently of later metadata state.
    pub fn len(&self) -> u64 {
        tracing::trace!("Reading accepted Storage range length");
        self.length
    }
    /// Supported range references always have positive length.
    pub fn is_empty(&self) -> bool {
        tracing::trace!("Checking accepted Storage range emptiness");
        self.length == 0
    }
    /// Read an exact positive slice relative to the range under a separate allocation maximum.
    /// # Errors
    /// Invalid rejects attempts to escape the range; Limit rejects allocation and Storage short/missing reads.
    pub fn read(&self, offset: u64, length: u64, maximum: usize) -> Result<Vec<u8>, Error> {
        tracing::trace!("Reading within an accepted immutable Storage subrange");
        if length == 0 || offset >= self.length || length > self.length - offset {
            return Err(Error::Invalid);
        }
        let absolute = self.offset.checked_add(offset).ok_or(Error::Invalid)?;
        self.handle.read(absolute, length, maximum)
    }
}
