//! Local single-identity selection and retained recipient policy; this does not transfer service state.
use super::{
    api::{IdentityId, PublicCredential},
    error::IdentityError,
};
use crate::modules::cryptography::{api::RecipientCredential, credential};
use std::collections::BTreeMap;

/// Relationship between two independently validated complete public credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyChange {
    /// Both pairs have unchanged public authority.
    Unchanged,
    /// Different signing authority creates a distinct identity with no inherited state.
    NewIdentity,
    /// The same ID has competing persistent encryption keys, unsupported by version one.
    InconsistentEncryptionKey,
}

/// Select exactly one validated local credential for subsequent node composition.
/// # Errors
/// Rejects zero or multiple entries and invalid public credentials; it never chooses the first entry.
pub fn select_configured(
    credentials: &[PublicCredential],
) -> Result<PublicCredential, IdentityError> {
    tracing::trace!("Selecting exactly one configured local identity");
    let [credential] = credentials else {
        return Err(IdentityError::InvalidInput);
    };
    validate(credential)?;
    Ok(*credential)
}

/// Classify complete credentials without granting replacement or rotation authority.
/// # Errors
/// Rejects an invalid ID/key relationship or binding before classifying any change.
pub fn classify(
    current: &PublicCredential,
    candidate: &PublicCredential,
) -> Result<KeyChange, IdentityError> {
    tracing::trace!("Classifying validated identity key changes");
    validate(current)?;
    validate(candidate)?;
    Ok(if current.identity.id() != candidate.identity.id() {
        KeyChange::NewIdentity
    } else if current.encryption_public_key != candidate.encryption_public_key {
        KeyChange::InconsistentEncryptionKey
    } else {
        KeyChange::Unchanged
    })
}

/// Require unchanged authority when a caller proposes a transparent configuration update.
/// # Errors
/// Returns Conflict for a different identity or encryption key; propagates credential validation failures.
pub fn require_unchanged(
    current: &PublicCredential,
    candidate: &PublicCredential,
) -> Result<(), IdentityError> {
    tracing::trace!("Rejecting transparent identity replacement or key rotation");
    match classify(current, candidate)? {
        KeyChange::Unchanged => Ok(()),
        KeyChange::NewIdentity | KeyChange::InconsistentEncryptionKey => {
            Err(IdentityError::Conflict)
        }
    }
}

/// Bounded local recipient pins; conflicting valid bindings never silently replace retained keys.
/// Ownership belongs to trusted composition; retaining a credential grants no peer trust.
pub struct RecipientPins {
    entries: BTreeMap<IdentityId, PublicCredential>,
    maximum: usize,
}
impl RecipientPins {
    /// Create an empty public-credential cache with an explicit nonzero entry limit.
    /// # Errors
    /// Returns InvalidInput for a zero entry budget.
    pub fn new(maximum: usize) -> Result<Self, IdentityError> {
        tracing::trace!("Creating a bounded retained recipient cache");
        if maximum == 0 {
            return Err(IdentityError::InvalidInput);
        }
        Ok(Self {
            entries: BTreeMap::new(),
            maximum,
        })
    }
    /// Validate and retain a recipient without choosing among conflicting encryption keys.
    /// # Errors
    /// Returns Conflict for a competing key, ResourceLimit for capacity, or a validation failure.
    pub fn retain(&mut self, candidate: PublicCredential) -> Result<(), IdentityError> {
        tracing::trace!("Retaining a validated recipient without implicit rotation");
        validate(&candidate)?;
        let id = candidate.identity.id();
        if let Some(current) = self.entries.get(&id) {
            return require_unchanged(current, &candidate);
        }
        if self.entries.len() >= self.maximum {
            return Err(IdentityError::ResourceLimit);
        }
        self.entries.insert(id, candidate);
        Ok(())
    }
    /// Copy a previously validated credential without implying reachability or possession.
    pub fn get(&self, id: IdentityId) -> Option<PublicCredential> {
        tracing::trace!("Reading a retained public recipient credential");
        self.entries.get(&id).copied()
    }
}

// Validate every public field through Cryptography before applying local configuration policy.
fn validate(value: &PublicCredential) -> Result<(), IdentityError> {
    tracing::trace!("Validating complete public authority before selection");
    credential::validate(&RecipientCredential {
        identity_id: value.identity.id().0,
        signing_public_key: value.identity.signing_public_key(),
        encryption_public_key: value.encryption_public_key,
        binding_signature: value.binding_signature,
    })?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/selection.rs"]
mod tests;
