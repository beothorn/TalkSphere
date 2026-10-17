//! Exact HELLO/AUTH payload shapes for handshake orchestration.
//! Selectors and widths are checked here; identity, signature and freshness are not.

/// A handshake payload has an incorrect width or unsupported selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakePayloadError {
    /// A complete payload must have exactly the specified number of bytes.
    Length { expected: usize, actual: usize },
    /// All version 1 selector bytes must be one.
    Selector { offset: usize, value: u8 },
}

/// Public HELLO material; none of these bytes prove identity ownership.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Hello {
    /// Advertised persistent Ed25519 public key.
    pub identity_public_key: [u8; 32],
    /// Fresh per-connection X25519 public key.
    pub ephemeral_public_key: [u8; 32],
    /// Fresh per-connection challenge; entropy is the Crypto adapter's obligation.
    pub nonce: [u8; 32],
}

impl Hello {
    /// Serialize the version 1 selectors and public fields into exactly 100 bytes.
    pub fn encode(&self) -> [u8; 100] {
        tracing::trace!(function = "Hello::encode", "Encoding HELLO fields");
        let mut bytes = [0; 100];
        bytes[..3].fill(1);
        bytes[3..35].copy_from_slice(&self.identity_public_key);
        bytes[35] = 1;
        bytes[36..68].copy_from_slice(&self.ephemeral_public_key);
        bytes[68..].copy_from_slice(&self.nonce);
        bytes
    }
    /// Check the exact HELLO shape, then copy its untrusted public fields.
    /// # Errors
    /// Rejects incorrect length or any selector other than version 1's value one.
    pub fn decode(bytes: &[u8]) -> Result<Self, HandshakePayloadError> {
        tracing::trace!(
            function = "Hello::decode",
            "Checking HELLO shape and selectors"
        );
        if bytes.len() != 100 {
            return Err(HandshakePayloadError::Length {
                expected: 100,
                actual: bytes.len(),
            });
        }
        for offset in [0, 1, 2, 35] {
            if bytes[offset] != 1 {
                return Err(HandshakePayloadError::Selector {
                    offset,
                    value: bytes[offset],
                });
            }
        }
        let mut hello = Self {
            identity_public_key: [0; 32],
            ephemeral_public_key: [0; 32],
            nonce: [0; 32],
        };
        hello.identity_public_key.copy_from_slice(&bytes[3..35]);
        hello.ephemeral_public_key.copy_from_slice(&bytes[36..68]);
        hello.nonce.copy_from_slice(&bytes[68..]);
        Ok(hello)
    }
}

/// An unverified 64-byte signature; validity is a separate Crypto operation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Auth(pub [u8; 64]);

impl Auth {
    /// Copy a complete signature without interpreting it as verified identity.
    /// # Errors
    /// Rejects any payload length other than 64 bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, HandshakePayloadError> {
        tracing::trace!(function = "Auth::decode", "Checking AUTH signature width");
        bytes
            .try_into()
            .map(Self)
            .map_err(|_| HandshakePayloadError::Length {
                expected: 64,
                actual: bytes.len(),
            })
    }
    /// Return the exact signature bytes for an untagged AUTH frame.
    pub fn encode(&self) -> [u8; 64] {
        tracing::trace!(function = "Auth::encode", "Encoding an AUTH signature");
        self.0
    }
}

#[cfg(test)]
#[path = "handshake/tests.rs"]
mod tests;
