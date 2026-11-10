//! Ordered detached body reception through Services-owned bounded body buffers.
use super::{
    Error,
    record::{DATA_MAX, Record},
    value::Pending,
};
use crate::modules::services::{body::Body, handler::Blob, value::Value};
use std::collections::BTreeMap;
/// One provisional logical value; only consuming finish can publish the assembled result.
pub struct Receiver {
    pending: Pending,
    index: usize,
    filled: u64,
    current: Option<Body>,
    complete: BTreeMap<u32, Blob>,
    failed: bool,
}
impl Receiver {
    /// Reserve no proportional body allocation; metadata has already admitted aggregate declared capacity.
    /// # Errors
    /// Rejects unrepresentable body sizes without constructing a receiver.
    pub fn new(pending: Pending) -> Result<Self, Error> {
        tracing::trace!("Creating ordered provisional peer body receiver");
        let current = next_body(&pending, 0)?;
        Ok(Self {
            pending,
            index: 0,
            filled: 0,
            current,
            complete: BTreeMap::new(),
            failed: false,
        })
    }
    /// Receive only the next declared ID and exact contiguous offset with a nonempty bounded chunk.
    /// Fragment logging is omitted to avoid peer-controlled per-byte diagnostic volume.
    /// # Errors
    /// Wrong record/order/length/UTF-8 poisons this value and discards provisional buffers.
    pub fn push(&mut self, record: &Record) -> Result<(), Error> {
        if self.failed {
            return Err(Error::Conflict);
        }
        let result = self.receive(record);
        if result.is_err() {
            self.failed = true;
            self.current = None;
            self.complete.clear();
        }
        result
    }
    // Advance between bodies only after exact completion and text validation; no partial Blob escapes.
    fn receive(&mut self, record: &Record) -> Result<(), Error> {
        let Record::Body { id, offset, data } = record else {
            return Err(Error::Invalid);
        };
        let descriptor = self
            .pending
            .descriptors()
            .get(self.index)
            .ok_or(Error::Invalid)?;
        if *id != descriptor.id || *offset != self.filled || !(1..=DATA_MAX).contains(&data.len()) {
            return Err(Error::Invalid);
        }
        self.current
            .as_mut()
            .ok_or(Error::Invalid)?
            .push(*offset, data)?;
        self.filled = self
            .filled
            .checked_add(data.len() as u64)
            .ok_or(Error::Invalid)?;
        if self.filled == descriptor.length {
            let bytes = self.current.take().ok_or(Error::Invalid)?.finish()?;
            if descriptor.text {
                std::str::from_utf8(bytes.bytes()).map_err(|_| Error::Invalid)?;
            }
            self.complete.insert(*id, bytes);
            self.index += 1;
            self.filled = 0;
            self.current = next_body(&self.pending, self.index)?;
        }
        Ok(())
    }
    /// Inspect whether all bodies are complete without exposing the provisional logical value.
    pub fn complete(&self) -> bool {
        tracing::trace!("Checking detached body completion");
        !self.failed && self.index == self.pending.descriptors().len()
    }
    /// Consume complete bodies into one owned Value; the exchange must separately require its marker and normal END.
    /// # Errors
    /// Failed or incomplete body reception returns Invalid and exposes no partial application value.
    pub fn finish(self) -> Result<Value, Error> {
        tracing::trace!("Publishing a complete locally assembled peer value");
        if !self.complete() {
            return Err(Error::Invalid);
        }
        self.pending.assemble(&self.complete)
    }
}
// Admit the next previously validated declaration without allocating its body payload.
fn next_body(pending: &Pending, index: usize) -> Result<Option<Body>, Error> {
    tracing::trace!("Preparing the next bounded peer body declaration");
    pending
        .descriptors()
        .get(index)
        .map(|d| {
            let maximum = usize::try_from(d.length).map_err(|_| Error::Limit)?;
            Body::new(d.length, None, maximum).map_err(Error::from)
        })
        .transpose()
}
