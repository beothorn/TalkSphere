//! OPEN protocol selection for stream routing; parsing does not imply support.
//! Callers validate the stream ID and integrity before using these borrowed fields.

/// A malformed selection, distinct from a valid unregistered protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenError {
    /// The payload length or declared identifier width is invalid.
    Length,
    /// The identifier violates the ASCII namespace grammar.
    Identifier,
    /// Protocol version zero is reserved.
    Version,
}

/// Validated protocol selection borrowed from a caller or complete OPEN payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Open<'a> {
    id: &'a str,
    version: u32,
}

impl<'a> Open<'a> {
    /// Validate a protocol identifier and nonzero version without allocating.
    /// # Errors
    /// Rejects identifiers outside 1–255 bytes, invalid grammar and version zero.
    pub fn new(id: &'a str, version: u32) -> Result<Self, OpenError> {
        tracing::trace!(function = "Open::new", "Validating protocol selection");
        let bytes = id.as_bytes();
        if !(1..=255).contains(&bytes.len()) {
            return Err(OpenError::Length);
        }
        let alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
        if !alphanumeric(bytes[0])
            || !alphanumeric(bytes[bytes.len() - 1])
            || !bytes
                .iter()
                .all(|&b| alphanumeric(b) || b == b'.' || b == b'-')
            || bytes.windows(2).any(|pair| pair == b"..")
        {
            return Err(OpenError::Identifier);
        }
        if version == 0 {
            return Err(OpenError::Version);
        }
        Ok(Self { id, version })
    }

    /// Decode an exact OPEN payload, retaining its identifier by reference.
    /// # Errors
    /// Rejects truncation, trailing bytes, invalid namespaces and version zero.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, OpenError> {
        tracing::trace!(function = "Open::decode", "Decoding stream selection");
        if bytes.len() < 7 {
            return Err(OpenError::Length);
        }
        let length = usize::from(u16::from_be_bytes([bytes[0], bytes[1]]));
        if !(1..=255).contains(&length) || bytes.len() != length + 6 {
            return Err(OpenError::Length);
        }
        let id = std::str::from_utf8(&bytes[2..2 + length]).map_err(|_| OpenError::Identifier)?;
        let offset = 2 + length;
        let version = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        Self::new(id, version)
    }

    /// Encode the validated identifier length, ASCII bytes and version in wire order.
    pub fn encode(&self) -> Vec<u8> {
        tracing::trace!(function = "Open::encode", "Encoding stream selection");
        let mut bytes = Vec::with_capacity(self.id.len() + 6);
        bytes.extend_from_slice(&(self.id.len() as u16).to_be_bytes());
        bytes.extend_from_slice(self.id.as_bytes());
        bytes.extend_from_slice(&self.version.to_be_bytes());
        bytes
    }

    /// Return the validated namespace; availability is a registry decision.
    pub fn id(&self) -> &'a str {
        tracing::trace!(function = "Open::id", "Reading selected namespace");
        self.id
    }

    /// Return the nonzero protocol version without negotiating compatibility.
    pub fn version(&self) -> u32 {
        tracing::trace!(function = "Open::version", "Reading selected version");
        self.version
    }
}

#[cfg(test)]
#[path = "open/tests.rs"]
mod tests;
