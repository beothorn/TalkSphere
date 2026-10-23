//! Linux owner-only anchored credential reads and exclusive store locking through safe rustix calls.
use super::super::{
    authority::{Administration, Authority, Capability},
    config::Limits,
    error::IdentityError,
    record::{CredentialRecord, RECORD_BYTES},
};
use rustix::fs::{self, CWD, FlockOperation, Mode, OFlags};
use std::{
    fs::File,
    io::Read,
    path::{Component, Path},
};

/// Protected local filesystem store; it protects against other users, not the same account or root.
/// The owner-only directory is locked until this value drops. No private bytes are logged.
pub struct FileStore {
    pub(super) directory: File,
    _lock: File,
    pub(super) authority: Authority,
    pub(super) limits: Limits,
}
impl FileStore {
    /// Open or create the final owner-only directory using no-follow anchored traversal.
    /// Returns a store and its administrative capability to trusted local composition.
    /// Parent directories must already exist; no parent traversal or symlink is followed.
    /// # Errors
    /// Rejects insecure permissions/ownership, unsupported path components, lock contention and I/O loss.
    pub fn open(
        path: &Path,
        limits: Limits,
    ) -> Result<(Self, Capability<Administration>), IdentityError> {
        tracing::trace!("Opening a protected local identity store");
        let directory = open_directory(path)?;
        validate_metadata(&directory, true)?;
        let lock = File::from(
            fs::openat(
                &directory,
                "writer.lock",
                OFlags::RDWR
                    | OFlags::CREATE
                    | OFlags::NOFOLLOW
                    | OFlags::CLOEXEC
                    | OFlags::NONBLOCK,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(io_error)?,
        );
        validate_metadata(&lock, false)?;
        fs::flock(&lock, FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| IdentityError::Conflict)?;
        let authority = Authority::new();
        let admin = authority.administration(limits.record_bytes())?;
        Ok((
            Self {
                directory,
                _lock: lock,
                authority,
                limits,
            },
            admin,
        ))
    }
    /// Read the existing complete credential record after checking local authority and protection.
    /// # Errors
    /// Never creates replacement keys; denies foreign authority, insecure files, oversized/corrupt records and unavailable storage.
    pub fn read(
        &self,
        admin: &Capability<Administration>,
    ) -> Result<CredentialRecord, IdentityError> {
        tracing::trace!("Reading existing protected identity credentials");
        self.authority.check_administration(admin, RECORD_BYTES)?;
        let bytes = self.read_bytes("identity.bin", self.limits.record_bytes())?;
        CredentialRecord::from_protected_bytes(bytes, self.limits)
    }
    // Read a fixed internal filename under the retained directory descriptor with bounded guarded staging.
    pub(super) fn read_bytes(
        &self,
        name: &str,
        maximum: usize,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, IdentityError> {
        tracing::trace!("Reading bounded protected local bytes");
        let file = File::from(
            fs::openat(
                &self.directory,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(io_error)?,
        );
        validate_metadata(&file, false)?;
        let size = usize::try_from(
            file.metadata()
                .map_err(|_| IdentityError::Unavailable)?
                .len(),
        )
        .map_err(|_| IdentityError::ResourceLimit)?;
        if size > maximum {
            return Err(IdentityError::ResourceLimit);
        }
        let bound = u64::try_from(maximum)
            .map_err(|_| IdentityError::ResourceLimit)?
            .checked_add(1)
            .ok_or(IdentityError::ResourceLimit)?;
        let mut bytes = zeroize::Zeroizing::new(Vec::with_capacity(size));
        file.take(bound)
            .read_to_end(&mut bytes)
            .map_err(|_| IdentityError::Unavailable)?;
        if bytes.len() > maximum {
            return Err(IdentityError::ResourceLimit);
        }
        Ok(bytes)
    }
}
// Walk each component through a retained directory descriptor; create only the final missing component.
fn open_directory(path: &Path) -> Result<File, IdentityError> {
    tracing::trace!("Opening a no-follow protected directory path");
    let components: Vec<_> = path.components().collect();
    if components.is_empty() {
        return Err(IdentityError::InvalidInput);
    }
    let root = if path.is_absolute() { "/" } else { "." };
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = File::from(fs::openat(CWD, root, flags, Mode::empty()).map_err(io_error)?);
    for (index, component) in components.iter().enumerate() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                let opened = match fs::openat(&directory, *name, flags, Mode::empty()) {
                    Ok(fd) => fd,
                    Err(rustix::io::Errno::NOENT) if index + 1 == components.len() => {
                        match fs::mkdirat(&directory, *name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
                            Ok(()) => {}
                            Err(rustix::io::Errno::EXIST) => {}
                            Err(error) => return Err(io_error(error)),
                        }
                        directory
                            .sync_all()
                            .map_err(|_| IdentityError::Unavailable)?;
                        fs::openat(&directory, *name, flags, Mode::empty()).map_err(io_error)?
                    }
                    Err(error) => return Err(io_error(error)),
                };
                directory = File::from(opened);
            }
            _ => return Err(IdentityError::InvalidInput),
        }
    }
    Ok(directory)
}
// Validate descriptor metadata rather than a separately resolved pathname before reading private material.
pub(super) fn validate_metadata(file: &File, directory: bool) -> Result<(), IdentityError> {
    tracing::trace!("Checking protected file ownership and permissions");
    let meta = fs::fstat(file).map_err(io_error)?;
    let expected = if directory { 0o040700 } else { 0o100600 };
    if meta.st_uid != rustix::process::geteuid().as_raw()
        || meta.st_mode & 0o177777 != expected
        || (!directory && meta.st_nlink != 1)
    {
        return Err(IdentityError::AccessDenied);
    }
    Ok(())
}
// Map platform failures without leaking local paths or guessing that an installation rolled back.
pub(super) fn io_error(error: rustix::io::Errno) -> IdentityError {
    tracing::trace!("Mapping protected filesystem failure");
    match error {
        rustix::io::Errno::ACCESS | rustix::io::Errno::PERM | rustix::io::Errno::LOOP => {
            IdentityError::AccessDenied
        }
        _ => IdentityError::Unavailable,
    }
}
