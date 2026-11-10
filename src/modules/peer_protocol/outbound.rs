//! Finite outbound record ownership across partial Transport admissions.
use super::{Error, record::Record};
use std::collections::VecDeque;
/// Application-owned pending bytes; advancing acknowledges only local Transport queue acceptance.
pub struct Outbound {
    queue: VecDeque<Vec<u8>>,
    offset: usize,
    bytes: usize,
    maximum: usize,
}
impl Outbound {
    /// Construct an empty finite byte queue.
    /// # Errors
    /// Zero capacity is Invalid.
    pub fn new(maximum: usize) -> Result<Self, Error> {
        tracing::trace!("Creating bounded Peer Protocol send queue");
        if maximum == 0 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            queue: VecDeque::new(),
            offset: 0,
            bytes: 0,
            maximum,
        })
    }
    /// Retain one complete encoded record if all bytes fit the remaining queue budget.
    /// # Errors
    /// Malformed records or exhausted capacity leave existing queued bytes unchanged.
    pub fn enqueue(&mut self, record: &Record) -> Result<(), Error> {
        tracing::trace!("Admitting complete outbound peer record");
        let length = record.wire_len()?;
        let total = self.bytes.checked_add(length).ok_or(Error::Limit)?;
        if total > self.maximum {
            return Err(Error::Limit);
        }
        let bytes = record.encode()?;
        self.bytes = total;
        self.queue.push_back(bytes);
        Ok(())
    }
    /// Borrow the next nonempty fragment without advancing; repeat this on Transport backpressure.
    pub fn front(&self, maximum: usize) -> Option<&[u8]> {
        tracing::trace!("Borrowing next pending peer record fragment");
        if maximum == 0 {
            return None;
        }
        self.queue
            .front()
            .map(|v| &v[self.offset..v.len().min(self.offset.saturating_add(maximum))])
    }
    /// Advance only bytes explicitly admitted by Transport, preserving order on partial progress.
    /// # Errors
    /// Zero or excess acknowledgement is Invalid and changes no state.
    pub fn advance(&mut self, count: usize) -> Result<(), Error> {
        tracing::trace!("Acknowledging local peer send queue progress");
        let front = self.queue.front().ok_or(Error::Invalid)?;
        if count == 0 || count > front.len() - self.offset {
            return Err(Error::Invalid);
        }
        self.offset += count;
        self.bytes -= count;
        if self.offset == front.len() {
            self.queue.pop_front();
            self.offset = 0;
        }
        Ok(())
    }
    /// Drop unsent local bytes on exchange cancellation; this makes no assertion about remote effects.
    pub fn cancel(&mut self) {
        tracing::trace!("Discarding unsent peer record bytes");
        self.queue.clear();
        self.offset = 0;
        self.bytes = 0;
    }
    /// Read aggregate pending bytes for admission and scheduling.
    pub fn pending(&self) -> usize {
        tracing::trace!("Reading queued peer record byte count");
        self.bytes
    }
}
