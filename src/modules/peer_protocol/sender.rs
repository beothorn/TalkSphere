//! Ordered immutable detached sources with queue-aware chunking and explicit local progress.
use super::{
    Error,
    contract::Limits,
    outbound::Outbound,
    record::{DATA_MAX, Record},
    value::Descriptor,
};
use crate::modules::services::handler::Blob;
/// Bounded immutable source ownership; queue admission never proves remote application receipt.
pub struct Sender {
    bodies: Vec<Blob>,
    index: usize,
    offset: usize,
}
impl Sender {
    /// Validate all source identities, exact lengths, aggregate capacity and final text before sending.
    /// # Errors
    /// Mismatched sources/declarations, malformed text or exhausted limits fail without queued bytes.
    pub fn new(
        bodies: Vec<Blob>,
        descriptors: &[Descriptor],
        limits: Limits,
    ) -> Result<Self, Error> {
        tracing::trace!("Admitting immutable ordered peer body sources");
        limits.validate()?;
        if bodies.len() != descriptors.len() {
            return Err(Error::Invalid);
        }
        if bodies.len() > limits.bodies {
            return Err(Error::Limit);
        }
        let mut remaining = limits.state_bytes;
        for (index, (body, d)) in bodies.iter().zip(descriptors).enumerate() {
            if u32::try_from(index).ok() != Some(d.id)
                || body.bytes().is_empty()
                || body.bytes().len() as u64 != d.length
            {
                return Err(Error::Invalid);
            }
            remaining = remaining
                .checked_sub(body.bytes().len())
                .ok_or(Error::Limit)?;
            if d.text {
                std::str::from_utf8(body.bytes()).map_err(|_| Error::Invalid)?;
            }
        }
        Ok(Self {
            bodies,
            index: 0,
            offset: 0,
        })
    }
    /// Enqueue at most one bounded body record, advancing only after queue admission succeeds.
    /// # Errors
    /// Zero chunk size is Invalid; backpressure is Limit and preserves source position for an explicit local retry.
    /// False means no bodies remain; this does not send terminal markers or END.
    pub fn enqueue_next(&mut self, queue: &mut Outbound, chunk: usize) -> Result<bool, Error> {
        tracing::trace!("Admitting the next ordered peer body chunk");
        if chunk == 0 {
            return Err(Error::Invalid);
        }
        let Some(body) = self.bodies.get(self.index) else {
            return Ok(false);
        };
        let count = (body.bytes().len() - self.offset)
            .min(chunk)
            .min(DATA_MAX)
            .min(queue.available().saturating_sub(17));
        if count == 0 {
            return Err(Error::Limit);
        }
        let id = u32::try_from(self.index).map_err(|_| Error::Limit)?;
        let record = Record::Body {
            id,
            offset: self.offset as u64,
            data: body.bytes()[self.offset..self.offset + count].to_vec(),
        };
        queue.enqueue(&record)?;
        self.offset += count;
        if self.offset == body.bytes().len() {
            self.index += 1;
            self.offset = 0;
        }
        Ok(true)
    }
    /// Observe whether all source data has entered the local queue; it may still be unsent or undelivered.
    pub fn queued(&self) -> bool {
        tracing::trace!("Checking local peer body queue completion");
        self.index == self.bodies.len()
    }
}
