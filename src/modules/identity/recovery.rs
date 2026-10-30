//! Privileged complete-key backup through protected local stores; application catalogs remain separate.
use super::{
    authority::{Backup, Capability},
    error::IdentityError,
    ports::KeyStore,
    provider::{Identity, IdentityHandle},
    record::{CredentialRecord, RECORD_BYTES},
};
use crate::modules::cryptography::ports::Entropy;

impl<S: KeyStore<Record = CredentialRecord>, E: Entropy> Identity<S, E> {
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
