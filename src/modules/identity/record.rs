//! Bounded implementation-specific credential staging; this is not a CR2SE wire or portable backup format.
use super::{config::Limits, error::IdentityError};
use crate::modules::cryptography::{
    error::CryptoError,
    identity_keys::{IdentityKeys, ProtectedKeySink},
};
use zeroize::Zeroizing;
const MAGIC: &[u8; 8] = b"TSID0001";
/// Exact local record size: magic, two private inputs, ID, two public keys and binding.
pub const RECORD_BYTES: usize = 232;

/// Guarded complete local record with no diagnostic or general byte-export interface.
pub struct CredentialRecord {
    bytes: Zeroizing<Vec<u8>>,
}
impl CredentialRecord {
    /// Take a bounded protected platform buffer, checking its local format before interpretation.
    /// # Errors
    /// Rejects excessive size, wrong exact length and unknown local format version.
    pub fn from_protected_bytes(
        bytes: Zeroizing<Vec<u8>>,
        limits: Limits,
    ) -> Result<Self, IdentityError> {
        tracing::trace!("Accepting bounded protected credential staging");
        if bytes.len() > limits.record_bytes() {
            return Err(IdentityError::ResourceLimit);
        }
        if bytes.len() != RECORD_BYTES {
            return Err(IdentityError::InvalidInput);
        }
        if &bytes[..8] != MAGIC {
            return Err(IdentityError::Unsupported);
        }
        Ok(Self { bytes })
    }
    /// Encode both private inputs and their verified public association into guarded local staging.
    /// The caller already owns private authority; only a protected platform store may receive this record.
    /// # Errors
    /// Rejects an insufficient record budget or public-derivation/storage-sink failure.
    pub fn encode(keys: &IdentityKeys, limits: Limits) -> Result<Self, IdentityError> {
        tracing::trace!("Encoding a complete guarded local credential record");
        if limits.record_bytes() < RECORD_BYTES {
            return Err(IdentityError::ResourceLimit);
        }
        let public = keys.credential()?;
        let mut sink = RecordSink(Zeroizing::new(Vec::with_capacity(RECORD_BYTES)));
        sink.0.extend_from_slice(MAGIC);
        keys.store(&mut sink)?;
        sink.0.extend_from_slice(&public.identity_id);
        sink.0.extend_from_slice(&public.signing_public_key);
        sink.0.extend_from_slice(&public.encryption_public_key);
        sink.0.extend_from_slice(&public.binding_signature);
        Self::from_protected_bytes(sink.0, limits)
    }
    /// Reconstruct protected pairs and reject every inconsistent cached public field.
    /// # Errors
    /// Returns IdentityMismatch for changed public association and propagates primitive errors.
    pub fn decode(&self) -> Result<IdentityKeys, IdentityError> {
        tracing::trace!("Validating complete stored identity key associations");
        let bytes = self.protected_bytes();
        let keys =
            IdentityKeys::import(bytes[8..40].to_vec().into(), bytes[40..72].to_vec().into())?;
        let public = keys.credential()?;
        if self.bytes[72..104] != public.identity_id
            || self.bytes[104..136] != public.signing_public_key
            || self.bytes[136..168] != public.encryption_public_key
            || self.bytes[168..232] != public.binding_signature
        {
            return Err(IdentityError::IdentityMismatch);
        }
        Ok(keys)
    }
    /// Return the fixed record length for authority and allocation checks without exporting its contents.
    pub fn len(&self) -> usize {
        tracing::trace!("Reading protected credential record size");
        self.bytes.len()
    }
    /// State whether the guarded record is empty; successful construction always returns false.
    pub fn is_empty(&self) -> bool {
        tracing::trace!("Checking protected credential staging presence");
        self.bytes.is_empty()
    }
    // Borrow bytes only for same-crate protected filesystem/backup adapters; never general module results.
    pub(crate) fn protected_bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing guarded record bytes for protected local storage");
        &self.bytes
    }
}
struct RecordSink(Zeroizing<Vec<u8>>);
impl ProtectedKeySink for RecordSink {
    // The caller already owns both private capabilities; the sink remains guarded local staging.
    fn authorize(&self) -> Result<(), CryptoError> {
        tracing::trace!("Authorizing guarded in-process record staging");
        Ok(())
    }
    // Append both inputs together without logging or publishing a partial record.
    fn store_pair(&mut self, signing: &[u8; 32], encryption: &[u8; 32]) -> Result<(), CryptoError> {
        tracing::trace!("Staging both persistent private inputs together");
        self.0.extend_from_slice(signing);
        self.0.extend_from_slice(encryption);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::envelope;
    // Local records preserve exact credentials, reject all structural truncations and detect cached-field mismatch.
    #[test]
    fn local_record_is_complete_bounded_and_consistent() {
        let limits = Limits::new(256, 4, 1024, 2048, RECORD_BYTES).unwrap();
        let keys = IdentityKeys::import(
            envelope("recipient Ed25519 seed").into(),
            envelope("recipient raw persistent X25519 private-key input").into(),
        )
        .unwrap();
        let record = CredentialRecord::encode(&keys, limits).unwrap();
        assert_eq!(record.len(), RECORD_BYTES);
        assert!(!record.is_empty());
        assert!(record.decode().unwrap().credential().unwrap() == keys.credential().unwrap());
        assert_eq!(&record.protected_bytes()[..8], MAGIC);
        for size in 0..RECORD_BYTES {
            assert!(
                CredentialRecord::from_protected_bytes(
                    record.protected_bytes()[..size].to_vec().into(),
                    limits
                )
                .is_err()
            );
        }
        let mut extra = record.protected_bytes().to_vec();
        extra.push(0);
        assert!(CredentialRecord::from_protected_bytes(extra.into(), limits).is_err());
        for index in [0, 7, 8, 40, 72, 104, 136, 168, 231] {
            let mut corrupt = Zeroizing::new(record.protected_bytes().to_vec());
            corrupt[index] ^= 0x10;
            match CredentialRecord::from_protected_bytes(corrupt, limits) {
                Ok(record) => assert!(record.decode().is_err()),
                Err(error) => assert_eq!(error, IdentityError::Unsupported),
            }
        }
        let small = Limits::new(58, 1, 1, 148, 1).unwrap();
        assert!(CredentialRecord::encode(&keys, small).is_err());
    }
}
