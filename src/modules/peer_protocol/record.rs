//! Incremental Peer Protocol framing, independent of Transport DATA boundaries and control interpretation.
use super::Error;
/// Largest permitted JSON control body in wire version one.
pub const CONTROL_MAX: usize = 1_048_576;
/// Largest permitted data segment within a detached body record.
pub const DATA_MAX: usize = 65_536;
/// One complete owned application record; framing alone does not authorize its exchange order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    /// Exact original JSON body bytes, retained for quote hashing.
    Control(Vec<u8>),
    /// Detached body ID, contiguous byte offset and nonempty data.
    Body {
        /// Traversal-assigned body ID.
        id: u32,
        /// Offset in logical bytes.
        offset: u64,
        /// Complete bounded data chunk.
        data: Vec<u8>,
    },
}
impl Record {
    /// Encode exact version-one framing after validating record-specific limits.
    /// # Errors
    /// Invalid rejects empty/oversized records before allocating the output.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        tracing::trace!("Encoding bounded Peer Protocol record");
        let (kind, length) = match self {
            Self::Control(bytes) if (1..=CONTROL_MAX).contains(&bytes.len()) => (1, bytes.len()),
            Self::Body { data, .. } if (1..=DATA_MAX).contains(&data.len()) => (2, 12 + data.len()),
            _ => return Err(Error::Invalid),
        };
        let mut bytes = Vec::with_capacity(5 + length);
        bytes.push(kind);
        bytes.extend_from_slice(&(length as u32).to_be_bytes());
        match self {
            Self::Control(body) => bytes.extend_from_slice(body),
            Self::Body { id, offset, data } => {
                bytes.extend_from_slice(&id.to_be_bytes());
                bytes.extend_from_slice(&offset.to_be_bytes());
                bytes.extend_from_slice(data);
            }
        }
        Ok(bytes)
    }
}
/// One stream direction's bounded parser; errors permanently poison this instance.
#[derive(Default)]
pub struct Decoder {
    header: [u8; 5],
    header_len: usize,
    length: usize,
    body: Vec<u8>,
    closed: bool,
}
impl Decoder {
    /// Admit at most one record and return consumed bytes plus an optional complete owned record.
    /// Caller must retain and process the remaining input before admitting more DATA.
    /// # Errors
    /// Invalid framing poisons the parser; calls after terminal/error state return Conflict.
    pub fn push(&mut self, input: &[u8]) -> Result<(usize, Option<Record>), Error> {
        tracing::trace!("Consuming bounded Peer Protocol framing input");
        if self.closed {
            return Err(Error::Conflict);
        }
        let result = self.consume(input);
        if result.is_err() {
            self.closed = true;
            self.body.clear();
        }
        result
    }
    // Validate a complete fixed header before reserving record payload storage.
    fn consume(&mut self, input: &[u8]) -> Result<(usize, Option<Record>), Error> {
        tracing::trace!("Advancing one record header and bounded body");
        let mut consumed = 0;
        if self.header_len < 5 {
            let count = (5 - self.header_len).min(input.len());
            self.header[self.header_len..self.header_len + count].copy_from_slice(&input[..count]);
            self.header_len += count;
            consumed += count;
            if self.header_len < 5 {
                return Ok((consumed, None));
            }
            self.length =
                u32::from_be_bytes(self.header[1..].try_into().map_err(|_| Error::Invalid)?)
                    as usize;
            match self.header[0] {
                1 if (1..=CONTROL_MAX).contains(&self.length) => {}
                2 if (13..=12 + DATA_MAX).contains(&self.length) => {}
                _ => return Err(Error::Invalid),
            }
            self.body.reserve_exact(self.length);
        }
        let count = (self.length - self.body.len()).min(input.len() - consumed);
        self.body
            .extend_from_slice(&input[consumed..consumed + count]);
        consumed += count;
        if self.body.len() != self.length {
            return Ok((consumed, None));
        }
        let body = std::mem::take(&mut self.body);
        let record = if self.header[0] == 1 {
            Record::Control(body)
        } else {
            Record::Body {
                id: u32::from_be_bytes(body[..4].try_into().map_err(|_| Error::Invalid)?),
                offset: u64::from_be_bytes(body[4..12].try_into().map_err(|_| Error::Invalid)?),
                data: body[12..].to_vec(),
            }
        };
        self.header_len = 0;
        self.length = 0;
        Ok((consumed, Some(record)))
    }
    /// Finish this direction only at a record boundary; exchange terminal markers are checked separately.
    /// # Errors
    /// A partial record is Invalid and no subsequent input is accepted.
    pub fn finish(&mut self) -> Result<(), Error> {
        tracing::trace!("Finishing Peer Protocol record direction");
        if self.closed {
            return Err(Error::Conflict);
        }
        self.closed = true;
        if self.header_len != 0 {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
