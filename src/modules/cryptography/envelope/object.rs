//! Complete-object bounds and borrowed views validated before size-dependent allocation.

use super::super::{config::ResourceLimits, error::CryptoError};
use super::header::{HEADER_BYTES, Header};

/// Structurally valid borrowed bytes; not authenticated or accepted plaintext.
pub struct ParsedObject<'a> {
    /// Decoded metadata for recipient checking and key derivation.
    pub header: Header,
    /// Exact original 132 bytes to authenticate, without reserialization.
    pub encoded_header: &'a [u8],
    /// Exactly the declared ciphertext and tag bytes.
    pub sealed: &'a [u8],
}

/// Compute a complete object's host-sized byte count from untrusted metadata.
/// Call this after reading the fixed header and before requesting the body.
/// # Errors
/// Rejects lengths below the tag minimum, overflow, host conversion or either local limit.
pub fn object_length(header: &Header, limits: ResourceLimits) -> Result<usize, CryptoError> {
    tracing::trace!("Checking encrypted-object lengths before body allocation");
    validate_lengths(header.sealed_length, limits)
}

/// Compute sealed and total lengths for a proposed plaintext without allocating.
/// # Errors
/// Rejects unsigned overflow, host-size conversion and plaintext/object budget excess.
pub fn lengths_for_plaintext(
    plaintext_length: u64,
    limits: ResourceLimits,
) -> Result<(u64, usize), CryptoError> {
    tracing::trace!("Checking proposed encryption sizes");
    let sealed = plaintext_length
        .checked_add(16)
        .ok_or(CryptoError::ResourceLimit)?;
    Ok((sealed, validate_lengths(sealed, limits)?))
}

/// Validate one complete bounded object and borrow its exact header and sealed body.
/// # Errors
/// Rejects truncation, trailing bytes, invalid headers or resource excess with no allocation.
pub fn parse(bytes: &[u8], limits: ResourceLimits) -> Result<ParsedObject<'_>, CryptoError> {
    tracing::trace!("Validating a complete encrypted object boundary");
    if bytes.len() < HEADER_BYTES {
        return Err(CryptoError::InvalidInput);
    }
    if bytes.len() > limits.maximum_object_bytes() {
        return Err(CryptoError::ResourceLimit);
    }
    let header = Header::decode(&bytes[..HEADER_BYTES])?;
    if bytes.len() != object_length(&header, limits)? {
        return Err(CryptoError::InvalidInput);
    }
    Ok(ParsedObject {
        header,
        encoded_header: &bytes[..HEADER_BYTES],
        sealed: &bytes[HEADER_BYTES..],
    })
}

// Shared arithmetic enforces both budgets without performing proportional work or allocation.
fn validate_lengths(sealed: u64, limits: ResourceLimits) -> Result<usize, CryptoError> {
    tracing::trace!("Validating ciphertext arithmetic and local resource budgets");
    let plaintext = sealed.checked_sub(16).ok_or(CryptoError::InvalidInput)?;
    let total = sealed
        .checked_add(HEADER_BYTES as u64)
        .ok_or(CryptoError::ResourceLimit)?;
    let total = usize::try_from(total).map_err(|_| CryptoError::ResourceLimit)?;
    let plaintext = usize::try_from(plaintext).map_err(|_| CryptoError::ResourceLimit)?;
    if total > limits.maximum_object_bytes() || plaintext > limits.maximum_plaintext_bytes() {
        return Err(CryptoError::ResourceLimit);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::envelope;
    // Every missing byte and an extra trailing byte are rejected without yielding body output.
    #[test]
    fn complete_object_views_and_truncation() {
        let mut bytes = envelope("132-byte header");
        bytes.extend(envelope("sealed ciphertext plus tag (46 bytes)"));
        let limits = ResourceLimits::new(30, 178).unwrap();
        let parsed = parse(&bytes, limits).unwrap();
        assert_eq!(parsed.encoded_header, envelope("132-byte header"));
        assert_eq!(
            parsed.sealed,
            envelope("sealed ciphertext plus tag (46 bytes)")
        );
        assert_eq!(lengths_for_plaintext(30, limits).unwrap(), (46, 178));
        for length in 0..bytes.len() {
            assert!(parse(&bytes[..length], limits).is_err());
        }
        bytes.push(0);
        assert!(parse(&bytes, ResourceLimits::new(100, 1000).unwrap()).is_err());
        assert!(parse(&bytes, limits).is_err());
        assert!(lengths_for_plaintext(31, limits).is_err());
        assert!(lengths_for_plaintext(30, ResourceLimits::new(29, 1000).unwrap()).is_err());
        assert!(lengths_for_plaintext(30, ResourceLimits::new(100, 177).unwrap()).is_err());
        assert_eq!(
            lengths_for_plaintext(0, ResourceLimits::new(1, 148).unwrap()).unwrap(),
            (16, 148)
        );
    }
    // Synthetic u64 sizes prove rejection before any attempt at huge allocation or read.
    #[test]
    fn unsigned_overflow_and_policy_boundaries() {
        let limits = ResourceLimits::new(usize::MAX, usize::MAX).unwrap();
        assert!(lengths_for_plaintext(u64::MAX, limits).is_err());
        let mut header = Header::decode(&envelope("132-byte header")).unwrap();
        for sealed in [0, 15, u64::MAX, u64::MAX - 131] {
            header.sealed_length = sealed;
            assert!(object_length(&header, limits).is_err());
        }
        header.sealed_length = 16;
        assert_eq!(object_length(&header, limits).unwrap(), 148);
    }
}
