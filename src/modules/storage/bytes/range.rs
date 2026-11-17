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
    // Verify a single coherent full-object byte sequence while retaining only the accepted range.
    // No full-object data escapes this internal provider check; temporary chunks remain bounded.
    pub(crate) fn verified(
        &self,
        expected: [u8; 32],
        maximum_range: usize,
        maximum_object: u64,
    ) -> Result<Vec<u8>, Error> {
        tracing::trace!(
            "Verifying retained Storage object while collecting only the accepted range"
        );
        use crate::modules::cryptography::primitives::hash::Sha256State;
        let length = usize::try_from(self.length).map_err(|_| Error::Limit)?;
        if length > maximum_range || self.handle.len() > maximum_object {
            return Err(Error::Limit);
        }
        let object_length = usize::try_from(self.handle.len()).map_err(|_| Error::Limit)?;
        let mut hash = Sha256State::new(object_length);
        let mut output = Vec::with_capacity(length);
        let end = self.offset.checked_add(self.length).ok_or(Error::Invalid)?;
        let mut position = 0;
        while position < self.handle.len() {
            let count = (self.handle.len() - position).min(65536);
            let chunk = self.handle.read(position, count, 65536)?;
            hash.update(&chunk).map_err(|_| Error::Limit)?;
            let chunk_end = position.checked_add(count).ok_or(Error::Invalid)?;
            let start = self.offset.max(position);
            let stop = end.min(chunk_end);
            if start < stop {
                let first = usize::try_from(start - position).map_err(|_| Error::Limit)?;
                let last = usize::try_from(stop - position).map_err(|_| Error::Limit)?;
                output.extend_from_slice(&chunk[first..last]);
            }
            position = chunk_end;
        }
        if output.len() != length || hash.finish() != expected {
            return Err(Error::Storage);
        }
        Ok(output)
    }
}
