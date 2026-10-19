//! Exact version-one 132-byte generic identity-envelope header, without body allocation.

use super::super::error::CryptoError;
/// Fixed generic envelope header size in bytes.
pub const HEADER_BYTES: usize = 132;
/// Authenticated metadata prefix used by the envelope's KDF.
pub const CONTEXT_BYTES: usize = 124;

/// Public untrusted metadata; successful decoding is not authentication.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Header {
    /// Intended recipient's 32-byte identity digest.
    pub recipient_id: [u8; 32],
    /// Intended recipient's persistent X25519 public key.
    pub recipient_key: [u8; 32],
    /// Per-object ephemeral X25519 public key.
    pub ephemeral_key: [u8; 32],
    /// Fresh per-object XChaCha20 nonce.
    pub nonce: [u8; 24],
    /// Ciphertext plus 16-byte tag length in wire bytes, excluding the header.
    pub sealed_length: u64,
}
impl Header {
    /// Encode the version-one suite and fixed fields with unsigned big-endian length.
    /// # Errors
    /// Returns `InvalidInput` for sealed lengths below the 16-byte tag minimum.
    pub fn encode(&self) -> Result<[u8; HEADER_BYTES], CryptoError> {
        tracing::trace!("Encoding a generic identity-encryption header");
        if self.sealed_length < 16 {
            return Err(CryptoError::InvalidInput);
        }
        let mut bytes = [0; HEADER_BYTES];
        bytes[..4].fill(1);
        bytes[4..36].copy_from_slice(&self.recipient_id);
        bytes[36..68].copy_from_slice(&self.recipient_key);
        bytes[68..100].copy_from_slice(&self.ephemeral_key);
        bytes[100..124].copy_from_slice(&self.nonce);
        bytes[124..].copy_from_slice(&self.sealed_length.to_be_bytes());
        Ok(bytes)
    }
    /// Decode exactly one header, rejecting unknown suite selectors without fallback.
    /// # Errors
    /// Returns `InvalidInput` for wrong size, unsupported selectors or sealed length below 16.
    pub fn decode(bytes: &[u8]) -> Result<Self, CryptoError> {
        tracing::trace!("Decoding untrusted identity-encryption metadata");
        if bytes.len() != HEADER_BYTES || bytes[..4] != [1; 4] {
            return Err(CryptoError::InvalidInput);
        }
        let header = Self {
            recipient_id: bytes[4..36]
                .try_into()
                .map_err(|_| CryptoError::InvalidInput)?,
            recipient_key: bytes[36..68]
                .try_into()
                .map_err(|_| CryptoError::InvalidInput)?,
            ephemeral_key: bytes[68..100]
                .try_into()
                .map_err(|_| CryptoError::InvalidInput)?,
            nonce: bytes[100..124]
                .try_into()
                .map_err(|_| CryptoError::InvalidInput)?,
            sealed_length: u64::from_be_bytes(
                bytes[124..132]
                    .try_into()
                    .map_err(|_| CryptoError::InvalidInput)?,
            ),
        };
        if header.sealed_length < 16 {
            return Err(CryptoError::InvalidInput);
        }
        Ok(header)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::envelope;
    // The literal fixed vector checks field offsets and byte-exact encoding independently.
    #[test]
    fn fixed_header_and_all_truncations() {
        let bytes = envelope("132-byte header");
        let header = Header::decode(&bytes).unwrap();
        assert_eq!(
            header.recipient_id.as_slice(),
            envelope("recipient CR2SE ID")
        );
        assert_eq!(
            header.recipient_key.as_slice(),
            envelope("recipient persistent X25519 public key")
        );
        assert_eq!(
            header.ephemeral_key.as_slice(),
            envelope("ephemeral X25519 public key")
        );
        assert_eq!(header.nonce.as_slice(), envelope("nonce"));
        assert_eq!(header.sealed_length, 46);
        assert_eq!(header.encode().unwrap().as_slice(), bytes);
        for length in 0..HEADER_BYTES {
            assert!(Header::decode(&bytes[..length]).is_err());
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(Header::decode(&extra).is_err());
        for index in 0..4 {
            for value in [0, 2, 255] {
                let mut changed = bytes.clone();
                changed[index] = value;
                assert!(Header::decode(&changed).is_err());
            }
        }
    }
    // Header codecs preserve large unsigned lengths; resource checks belong to object parsing.
    #[test]
    fn sealed_length_boundaries() {
        let mut header = Header::decode(&envelope("132-byte header")).unwrap();
        for length in [16, u64::MAX] {
            header.sealed_length = length;
            assert_eq!(
                Header::decode(&header.encode().unwrap())
                    .unwrap()
                    .sealed_length,
                length
            );
        }
        for length in [0u64, 15] {
            header.sealed_length = length;
            assert!(header.encode().is_err());
            let mut bytes = envelope("132-byte header");
            bytes[124..].copy_from_slice(&length.to_be_bytes());
            assert!(Header::decode(&bytes).is_err());
        }
    }
}
