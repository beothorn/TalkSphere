//! Immutable application policy for Identity inputs and local storage.
use super::error::IdentityError;

/// Independently enforced byte/count limits; these are not CR2SE wire maxima.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    text_bytes: usize,
    contacts: usize,
    signing_bytes: usize,
    envelope_bytes: usize,
    record_bytes: usize,
}
impl Limits {
    /// Validate local budgets before exposing any identity operation.
    /// Counts are contacts; all other arguments are bytes from trusted composition.
    /// # Errors
    /// Rejects zero capacities, text below 58 bytes or envelopes below 148 bytes.
    pub fn new(
        text_bytes: usize,
        contacts: usize,
        signing_bytes: usize,
        envelope_bytes: usize,
        record_bytes: usize,
    ) -> Result<Self, IdentityError> {
        tracing::trace!("Validating Identity resource budgets");
        if text_bytes < 58
            || contacts == 0
            || signing_bytes == 0
            || envelope_bytes < 148
            || record_bytes == 0
        {
            return Err(IdentityError::InvalidInput);
        }
        Ok(Self {
            text_bytes,
            contacts,
            signing_bytes,
            envelope_bytes,
            record_bytes,
        })
    }
    /// Return the immutable budget for text bytes.
    pub fn text_bytes(&self) -> usize {
        tracing::trace!("Reading the Identity text bytes budget");
        self.text_bytes
    }
    /// Return the immutable budget for configured contacts.
    pub fn contacts(&self) -> usize {
        tracing::trace!("Reading the Identity configured contacts budget");
        self.contacts
    }
    /// Return the immutable budget for signed bytes.
    pub fn signing_bytes(&self) -> usize {
        tracing::trace!("Reading the Identity signed bytes budget");
        self.signing_bytes
    }
    /// Return the immutable budget for complete envelope bytes.
    pub fn envelope_bytes(&self) -> usize {
        tracing::trace!("Reading the Identity complete envelope bytes budget");
        self.envelope_bytes
    }
    /// Return the immutable budget for local record bytes.
    pub fn record_bytes(&self) -> usize {
        tracing::trace!("Reading the Identity local record bytes budget");
        self.record_bytes
    }
}
