//! Explicit local dependencies; adapters own protected storage and algorithms rather than exposing private state.
use super::{
    api::{Contact, IdentityId},
    error::IdentityError,
};

/// Cryptographic boundary needed for deterministic ID derivation and proof validation.
pub trait Cryptography {
    /// Hash exact caller bytes; providers enforce their local input limit.
    /// # Errors
    /// Reports resource or dependency failures without fabricated output.
    fn sha256(&self, input: &[u8]) -> Result<[u8; 32], IdentityError>;
    /// Validate canonical signing-key encoding and reject small-order points.
    /// # Errors
    /// Rejects invalid keys and unavailable dependencies.
    fn validate_signing_key(&self, input: &[u8]) -> Result<(), IdentityError>;
    /// Verify ordinary Ed25519 over exact bounded bytes under the CR2SE profile.
    /// # Errors
    /// Returns invalid-proof, malformed-input or dependency failures.
    fn verify(&self, key: &[u8], message: &[u8], signature: &[u8]) -> Result<(), IdentityError>;
}

/// Atomic protected persistence; record representation is owned by the platform adapter.
pub trait KeyStore {
    /// Local authorized administrative capability; peers cannot supply it.
    type Administration;
    /// Protected record, with scoped cleanup and no required serialization or Debug.
    type Record;
    /// Install both keys and their association once, with durable success.
    /// # Errors
    /// Denial precedes effects; existing state conflicts; uncertain commit must be reconciled.
    fn install(
        &mut self,
        authority: &Self::Administration,
        record: Self::Record,
    ) -> Result<(), IdentityError>;
    /// Open the existing protected record without creating replacement keys.
    /// # Errors
    /// Reports denied/unavailable/corrupt records, never fabricated keys.
    fn open(&self, authority: &Self::Administration) -> Result<Self::Record, IdentityError>;
}

/// Configured-contact owner; snapshots never expose mutable backing state.
pub trait ContactStore {
    /// Local authority allowed to inspect or update these configured contacts.
    type Authority;
    /// Return a copied contact hint for an exact binary ID.
    /// # Errors
    /// Rejects unauthorized reads without exposing protected existence.
    fn get(
        &self,
        authority: &Self::Authority,
        id: IdentityId,
    ) -> Result<Option<Contact>, IdentityError>;
    /// Replace one contact without changing the ID to which it is bound.
    /// # Errors
    /// Rejects denied, oversized or unavailable writes; durable adapters report uncertain outcomes.
    fn put(&mut self, authority: &Self::Authority, contact: Contact) -> Result<(), IdentityError>;
}
