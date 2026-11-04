//! Privileged complete-key backup through protected local stores; application catalogs remain separate.
use super::{
    api::{IdentityId, PublicCredential},
    authority::{Backup, Capability, Import},
    error::IdentityError,
    ports::KeyStore,
    provider::{Identity, IdentityHandle},
    record::{CredentialRecord, RECORD_BYTES},
};
use crate::modules::cryptography::ports::Entropy;

impl<S: KeyStore<Record = CredentialRecord>, E: Entropy> Identity<S, E> {
    /// Grant identity-specific import authority after checking destination administration.
    /// # Errors
    /// Denies unauthorized administration and invalid or revoked capability budgets.
    pub fn grant_import(
        &self,
        admin: &S::Administration,
        expected: IdentityId,
        maximum_bytes: usize,
    ) -> Result<Capability<Import>, IdentityError> {
        tracing::trace!("Granting explicit local identity import authority");
        self.store.authorize(admin, RECORD_BYTES)?;
        self.authority.grant(expected, maximum_bytes)
    }

    /// Restore both validated private pairs into a new protected store without replacing existing keys.
    /// # Errors
    /// Rejects denied authority, unexpected identity, corrupt association and destination conflicts.
    /// Uncertain installation requires destination reconciliation rather than regeneration or retry.
    pub fn restore<D: KeyStore<Record = CredentialRecord>>(
        &mut self,
        admin: &S::Administration,
        capability: &Capability<Import>,
        expected: IdentityId,
        source: &D,
        source_admin: &D::Administration,
    ) -> Result<(IdentityHandle, PublicCredential), IdentityError> {
        tracing::trace!("Restoring complete validated identity authority from protected backup");
        self.store.authorize(admin, RECORD_BYTES)?;
        self.authority.check(capability, expected, RECORD_BYTES)?;
        source.authorize(source_admin, RECORD_BYTES)?;
        let record = source.open(source_admin)?;
        if record.len() > self.limits.record_bytes() {
            return Err(IdentityError::ResourceLimit);
        }
        let result = self.handle(record.decode()?)?;
        if result.1.identity.id() != expected {
            return Err(IdentityError::IdentityMismatch);
        }
        self.store.install(admin, record)?;
        Ok(result)
    }

    /// Snapshot both private pairs and their association into an authorized protected destination.
    /// The destination must honor KeyStore's protection, atomicity and no-overwrite contract.
    /// # Errors
    /// Denies foreign handles/capabilities or destination authority before private staging; preserves
    /// destination conflict, I/O and uncertain-outcome failures without exporting bytes or retrying.
    pub fn backup<D: KeyStore<Record = CredentialRecord>>(
        &self,
        handle: &IdentityHandle,
        capability: &Capability<Backup>,
        destination: &mut D,
        destination_admin: &D::Administration,
    ) -> Result<(), IdentityError> {
        tracing::trace!("Backing up a complete identity into a protected local destination");
        self.check_handle(handle)?;
        self.authority
            .check(capability, handle.public.identity.id(), RECORD_BYTES)?;
        destination.authorize(destination_admin, RECORD_BYTES)?;
        destination.install(
            destination_admin,
            CredentialRecord::encode(&handle.keys, self.limits)?,
        )
    }
}
