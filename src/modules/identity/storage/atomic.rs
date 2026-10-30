//! Atomic protected installation and snapshot publication under the store's exclusive writer lock.
use super::file::{FileStore, io_error, validate_metadata};
use crate::modules::identity::{
    authority::{Administration, Capability},
    error::IdentityError,
    ports::KeyStore,
    record::CredentialRecord,
};
use rustix::fs::{self, AtFlags, Mode, OFlags, RenameFlags};
use std::{fs::File, io::Write};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Cut {
    Created,
    Written,
    Synced,
    Installed,
}
impl FileStore {
    /// Revalidate existing credentials and finish directory durability after an uncertain installation.
    /// # Errors
    /// Reports authorization, corruption or I/O failure without creating or replacing any key.
    pub fn reconcile(
        &self,
        admin: &Capability<Administration>,
    ) -> Result<CredentialRecord, IdentityError> {
        tracing::trace!("Reconciling an existing identity installation without regeneration");
        let record = self.read(admin)?;
        let _keys = record.decode()?;
        self.directory
            .sync_all()
            .map_err(|_| IdentityError::OutcomeUnknown)?;
        Ok(record)
    }
    // Publish one fixed internal record; only contacts may replace a prior snapshot.
    pub(crate) fn write_record(
        &self,
        name: &str,
        bytes: &[u8],
        replace: bool,
    ) -> Result<(), IdentityError> {
        tracing::trace!("Publishing a complete protected local record atomically");
        if !matches!(
            (name, replace),
            ("identity.bin", false) | ("contacts.bin", true)
        ) {
            return Err(IdentityError::InvalidInput);
        }
        if bytes.len() > self.limits.record_bytes() {
            return Err(IdentityError::ResourceLimit);
        }
        let staging = if name == "identity.bin" {
            "identity.pending"
        } else {
            "contacts.pending"
        };
        let mut file = File::from(
            fs::openat(
                &self.directory,
                staging,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(io_error)?,
        );
        let mut guard = Staging {
            directory: &self.directory,
            name: staging,
            installed: false,
        };
        validate_metadata(&file, false)?;
        self.cut(Cut::Created)?;
        file.write_all(bytes)
            .map_err(|_| IdentityError::Unavailable)?;
        self.cut(Cut::Written)?;
        file.sync_all().map_err(|_| IdentityError::Unavailable)?;
        self.cut(Cut::Synced)?;
        let flags = if replace {
            RenameFlags::empty()
        } else {
            RenameFlags::NOREPLACE
        };
        fs::renameat_with(&self.directory, staging, &self.directory, name, flags).map_err(
            |error| {
                if error == rustix::io::Errno::EXIST {
                    IdentityError::Conflict
                } else {
                    io_error(error)
                }
            },
        )?;
        guard.installed = true;
        self.cut(Cut::Installed)
            .map_err(|_| IdentityError::OutcomeUnknown)?;
        self.directory
            .sync_all()
            .map_err(|_| IdentityError::OutcomeUnknown)?;
        Ok(())
    }
    // Fault cuts are observable only in tests; normal production calls contain no injected failure.
    #[cfg(not(test))]
    fn cut(&self, _cut: Cut) -> Result<(), IdentityError> {
        tracing::trace!("Checking protected installation progress");
        Ok(())
    }
}
impl KeyStore for FileStore {
    type Administration = Capability<Administration>;
    type Record = CredentialRecord;
    /// Validate local administration before a caller generates keys or inspects the store.
    fn authorize(
        &self,
        authority: &Self::Administration,
        bytes: usize,
    ) -> Result<(), IdentityError> {
        tracing::trace!("Checking protected-store administrative authority before work");
        self.authority.check_administration(authority, bytes)
    }
    /// Commit a validated complete association without replacing an existing identity.
    fn install(
        &mut self,
        authority: &Self::Administration,
        record: CredentialRecord,
    ) -> Result<(), IdentityError> {
        tracing::trace!("Installing both identity keys under local administrative authority");
        self.authority
            .check_administration(authority, record.len())?;
        let _validated = record.decode()?;
        self.write_record("identity.bin", record.protected_bytes(), false)
    }
    /// Read an existing protected record; callers validate its complete association before use.
    fn open(&self, authority: &Self::Administration) -> Result<CredentialRecord, IdentityError> {
        tracing::trace!("Opening an existing identity through the protected store port");
        self.read(authority)
    }
}
struct Staging<'a> {
    directory: &'a File,
    name: &'a str,
    installed: bool,
}
impl Drop for Staging<'_> {
    // Remove incomplete private staging on normal error exits; restart handles process-abandoned staging.
    fn drop(&mut self) {
        tracing::trace!("Releasing a protected installation staging file");
        if !self.installed && fs::unlinkat(self.directory, self.name, AtFlags::empty()).is_err() {
            tracing::warn!(
                "Protected staging removal failed; store restart must reconcile it before new work"
            );
        }
    }
}
// Under the exclusive writer lock, remove only known protected orphan staging names from a prior process.
pub(super) fn cleanup_staging(directory: &File) -> Result<(), IdentityError> {
    tracing::trace!("Cleaning abandoned protected staging under the store lock");
    for name in ["identity.pending", "contacts.pending"] {
        let file = match fs::openat(
            directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => File::from(fd),
            Err(rustix::io::Errno::NOENT) => continue,
            Err(error) => return Err(io_error(error)),
        };
        validate_metadata(&file, false)?;
        fs::unlinkat(directory, name, AtFlags::empty()).map_err(io_error)?;
        directory
            .sync_all()
            .map_err(|_| IdentityError::Unavailable)?;
    }
    Ok(())
}
