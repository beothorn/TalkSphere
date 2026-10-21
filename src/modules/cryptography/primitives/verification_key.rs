//! Canonical Edwards point validation shared by Identity keys and Ed25519 signature R values.
use crate::modules::cryptography::error::CryptoError;
use curve25519_dalek::edwards::CompressedEdwardsY;

/// Canonical non-small-order verification point; it grants no identity trust or possession.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerificationKey([u8; 32]);
impl VerificationKey {
    /// Validate exact encoding, curve membership and the CR2SE small-order restriction.
    /// Mixed-order points are permitted; signature verification decides their exact equation.
    /// # Errors
    /// Returns InvalidInput for width or decoding errors and AuthenticationFailed for
    /// noncanonical or small-order points; no secret operation or allocation occurs.
    pub fn parse(encoded: &[u8]) -> Result<Self, CryptoError> {
        tracing::trace!("Validating a canonical non-small-order verification point");
        let bytes: [u8; 32] = encoded.try_into().map_err(|_| CryptoError::InvalidInput)?;
        let point = CompressedEdwardsY(bytes)
            .decompress()
            .ok_or(CryptoError::InvalidInput)?;
        if point.compress().to_bytes() != bytes || point.is_small_order() {
            return Err(CryptoError::AuthenticationFailed);
        }
        Ok(Self(bytes))
    }
    /// Return the original canonical public bytes; no private authority is exposed.
    pub fn to_bytes(self) -> [u8; 32] {
        tracing::trace!("Reading validated verification point bytes");
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::{hex, transport};
    use curve25519_dalek::constants::{ED25519_BASEPOINT_POINT, EIGHT_TORSION};

    // Honest and mixed-order keys are allowed, but small-order and noncanonical encodings are not.
    #[test]
    fn canonical_profile_rejects_invalid_keys_without_subgroup_overrestriction() {
        for role in ["initiator", "acceptor"] {
            let key = transport(&format!("{role} Ed25519 public key"));
            assert_eq!(
                VerificationKey::parse(&key).unwrap().to_bytes().as_slice(),
                key
            );
        }
        for torsion in EIGHT_TORSION {
            assert!(VerificationKey::parse(torsion.compress().as_bytes()).is_err());
        }
        for value in [
            "edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
            "0100000000000000000000000000000000000000000000000000000000000080",
        ] {
            assert!(VerificationKey::parse(&hex(value)).is_err());
        }
        for length in [0, 31, 33] {
            assert_eq!(
                VerificationKey::parse(&vec![0; length]),
                Err(CryptoError::InvalidInput)
            );
        }
        let mixed = (ED25519_BASEPOINT_POINT + EIGHT_TORSION[1])
            .compress()
            .to_bytes();
        assert_eq!(VerificationKey::parse(&mixed).unwrap().to_bytes(), mixed);
        // Some canonical y encodings are not curve points; find one deterministically.
        let invalid = (0u8..=255)
            .map(|b| {
                let mut x = [0; 32];
                x[0] = b;
                x
            })
            .find(|x| CompressedEdwardsY(*x).decompress().is_none())
            .unwrap();
        assert_eq!(
            VerificationKey::parse(&invalid),
            Err(CryptoError::InvalidInput)
        );
    }
}
