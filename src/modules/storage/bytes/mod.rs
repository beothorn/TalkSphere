//! Protected Linux upload staging and immutable positional byte reads; no lease or payment authority.
mod fs_support;
mod io;
mod snapshot;
use crate::modules::{cryptography::primitives::hash::Sha256State, services::Error};
use rustix::fs::{self, AtFlags, FlockOperation, Mode, OFlags, RenameFlags};
use std::{fs::File, os::unix::fs::FileExt, path::Path, sync::Arc};

struct Directory {
    file: File,
    _lock: File,
    #[cfg(test)]
    snapshot_fault: std::sync::atomic::AtomicU8,
}
/// Exclusive protected byte owner; clones share one writer lock and descriptor anchor.
#[derive(Clone)]
pub struct Store(Arc<Directory>);
/// Provisional exact-length upload; incomplete bytes are never returned as a complete handle.
pub struct Upload {
    directory: Arc<Directory>,
    file: File,
    id: [u8; 32],
    length: u64,
    written: u64,
    hash: Option<Sha256State>,
    poisoned: bool,
    installed: bool,
    #[cfg(test)]
    fault: Option<u8>,
}
/// Immutable owner-retained byte reference; its open descriptor survives logical unlink.
#[derive(Clone)]
pub struct Handle {
    directory: Arc<Directory>,
    file: Arc<File>,
    id: [u8; 32],
    length: u64,
}
impl Store {
    /// Open or explicitly create the final owner-only directory and acquire its exclusive writer lock.
    /// # Errors
    /// Invalid rejects unsafe paths, Denied unsafe ownership/modes or competing writers, Storage filesystem failures.
    pub fn open(path: &Path, create: bool) -> Result<Self, Error> {
        tracing::trace!("Opening protected Storage byte staging");
        let file = fs_support::directory(path, create)?;
        fs_support::protected(&file, true)?;
        let lock = File::from(
            fs::openat(
                &file,
                "writer.lock",
                OFlags::RDWR
                    | OFlags::CREATE
                    | OFlags::NOFOLLOW
                    | OFlags::CLOEXEC
                    | OFlags::NONBLOCK,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|_| Error::Storage)?,
        );
        fs_support::protected(&lock, false)?;
        fs::flock(&lock, FlockOperation::NonBlockingLockExclusive).map_err(|_| Error::Denied)?;
        Ok(Self(Arc::new(Directory {
            file,
            _lock: lock,
            #[cfg(test)]
            snapshot_fault: std::sync::atomic::AtomicU8::new(0),
        })))
    }
    /// Begin one locally authorized positive upload after checking its declared length against the caller's quota.
    /// IDs originate from trusted owner randomness, never peer filesystem names.
    /// # Errors
    /// Limit rejects excessive/address-space lengths before allocation; Conflict rejects reused staging IDs.
    /// Invalid rejects zero length; Storage/Denied preserve platform failures.
    pub fn begin(&self, id: [u8; 32], length: u64, maximum: u64) -> Result<Upload, Error> {
        tracing::trace!("Beginning bounded provisional Storage upload");
        if length == 0 {
            return Err(Error::Invalid);
        }
        if length > maximum {
            return Err(Error::Limit);
        }
        let bound = usize::try_from(length).map_err(|_| Error::Limit)?;
        let file = File::from(
            fs::openat(
                &self.0.file,
                name(id, true),
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|e| {
                if e == rustix::io::Errno::EXIST {
                    Error::Conflict
                } else {
                    Error::Storage
                }
            })?,
        );
        fs_support::protected(&file, false)?;
        Ok(Upload {
            directory: self.0.clone(),
            file,
            id,
            length,
            written: 0,
            hash: Some(Sha256State::new(bound)),
            poisoned: false,
            installed: false,
            #[cfg(test)]
            fault: None,
        })
    }
    /// Reopen a durable immutable object using owner metadata and exact expected length.
    /// This reference authorizes no remote lease access; callers independently authorize requester identity.
    /// # Errors
    /// Storage/Denied reject unavailable or unsafe objects; Invalid rejects mismatched lengths.
    pub fn reopen(&self, id: [u8; 32], length: u64) -> Result<Handle, Error> {
        tracing::trace!("Reopening retained immutable Storage bytes");
        let file = File::from(
            fs::openat(
                &self.0.file,
                name(id, false),
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(|_| Error::Storage)?,
        );
        fs_support::protected(&file, false)?;
        if length == 0 || file.metadata().map_err(|_| Error::Storage)?.len() != length {
            return Err(Error::Invalid);
        }
        Ok(Handle {
            directory: self.0.clone(),
            file: Arc::new(file),
            id,
            length,
        })
    }
    /// Explicitly discard orphan provisional bytes after the owner has established they are uncommitted.
    /// # Errors
    /// Unsafe entries fail Denied; filesystem failures fail Storage or Unknown after unlink.
    pub fn discard(&self, id: [u8; 32]) -> Result<(), Error> {
        tracing::trace!("Discarding owner-confirmed provisional Storage bytes");
        let path = name(id, true);
        let file = match fs::openat(
            &self.0.file,
            &path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(fd) => File::from(fd),
            Err(rustix::io::Errno::NOENT) => return Ok(()),
            Err(_) => return Err(Error::Storage),
        };
        fs_support::protected(&file, false)?;
        fs::unlinkat(&self.0.file, path, AtFlags::empty()).map_err(|_| Error::Storage)?;
        self.0.file.sync_all().map_err(|_| Error::Unknown)
    }
}
impl Upload {
    /// Append only the exact next offset within the declared size, poisoning this attempt on any failed write.
    /// # Errors
    /// Invalid rejects order, length or poisoned attempts; Storage reports incomplete platform writes.
    pub fn append(&mut self, offset: u64, bytes: &[u8]) -> Result<(), Error> {
        tracing::trace!("Appending an ordered bounded Storage upload chunk");
        if self.poisoned
            || offset != self.written
            || u64::try_from(bytes.len()).map_err(|_| Error::Limit)? > self.length - self.written
        {
            self.poisoned = true;
            return Err(Error::Invalid);
        }
        let result = self
            .cut(1)
            .and_then(|_| io::write_exact(&mut self.file, bytes));
        if let Err(e) = result {
            self.poisoned = true;
            return Err(e);
        }
        self.hash
            .as_mut()
            .ok_or(Error::Invalid)?
            .update(bytes)
            .map_err(|_| Error::Limit)?;
        self.written += bytes.len() as u64;
        Ok(())
    }
    /// Validate complete length/hash, sync bytes, install without overwriting any object and sync the directory.
    /// A complete byte object is not an active lease; metadata participant commitment remains separate.
    /// # Errors
    /// Invalid rejects incomplete/corrupt attempts; Conflict rejects reused object IDs; Storage precedes install,
    /// and Unknown preserves possible durable installation after rename.
    pub fn finish(mut self, expected: [u8; 32]) -> Result<Handle, Error> {
        tracing::trace!("Durably installing a complete validated Storage byte object");
        if self.poisoned
            || self.written != self.length
            || self.hash.take().ok_or(Error::Invalid)?.finish() != expected
        {
            return Err(Error::Invalid);
        }
        self.cut(2)?;
        self.file.sync_all().map_err(|_| Error::Storage)?;
        self.cut(3)?;
        fs::renameat_with(
            &self.directory.file,
            name(self.id, true),
            &self.directory.file,
            name(self.id, false),
            RenameFlags::NOREPLACE,
        )
        .map_err(|e| {
            if e == rustix::io::Errno::EXIST {
                Error::Conflict
            } else {
                Error::Storage
            }
        })?;
        self.installed = true;
        self.cut(4)?;
        self.directory.file.sync_all().map_err(|_| Error::Unknown)?;
        self.cut(5)?;
        let file = self.file.try_clone().map_err(|_| Error::Unknown)?;
        Ok(Handle {
            directory: self.directory.clone(),
            file: Arc::new(file),
            id: self.id,
            length: self.length,
        })
    }
    // Test-only faults separate failures before installation from lost replies after possible commitment.
    fn cut(&self, point: u8) -> Result<(), Error> {
        tracing::trace!("Checking Storage upload durability progress");
        #[cfg(test)]
        if self.fault == Some(point) {
            return Err(if point >= 4 {
                Error::Unknown
            } else {
                Error::Storage
            });
        }
        let _ = point;
        Ok(())
    }
}
impl Drop for Upload {
    // Best-effort provisional cleanup never removes a possibly installed immutable object.
    fn drop(&mut self) {
        tracing::trace!("Releasing provisional Storage upload ownership");
        if !self.installed {
            let _ = fs::unlinkat(&self.directory.file, name(self.id, true), AtFlags::empty());
        }
    }
}
impl Handle {
    /// Return the opaque owner object ID without exposing filesystem names or paths.
    pub fn id(&self) -> [u8; 32] {
        tracing::trace!("Reading immutable Storage object identity");
        self.id
    }
    /// Return the validated complete logical length, in bytes.
    pub fn len(&self) -> u64 {
        tracing::trace!("Reading immutable Storage object length");
        self.length
    }
    /// Complete byte objects are always positive in Storage version one.
    pub fn is_empty(&self) -> bool {
        tracing::trace!("Checking immutable Storage object emptiness");
        self.length == 0
    }
    /// Read an exact positive range with independent finite allocation bounds and cursor-free positional I/O.
    /// # Errors
    /// Invalid rejects out-of-object ranges; Limit rejects excessive/address-space allocation; Storage rejects short reads.
    pub fn read(&self, offset: u64, length: u64, maximum: usize) -> Result<Vec<u8>, Error> {
        tracing::trace!("Reading exact bounded immutable Storage range");
        if length == 0 || offset >= self.length || length > self.length - offset {
            return Err(Error::Invalid);
        }
        let length = usize::try_from(length).map_err(|_| Error::Limit)?;
        if length > maximum {
            return Err(Error::Limit);
        }
        let mut bytes = vec![0; length];
        self.file
            .read_exact_at(&mut bytes, offset)
            .map_err(|_| Error::Storage)?;
        let _ = &self.directory;
        Ok(bytes)
    }
}
// Encode a fixed owner-issued identifier; peer text never becomes a path component.
fn name(id: [u8; 32], pending: bool) -> String {
    tracing::trace!("Selecting an owner-controlled Storage object entry");
    use std::fmt::Write as _;
    let mut output = String::from(if pending { "upload-" } else { "object-" });
    for byte in id {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}
#[cfg(test)]
pub(super) mod tests;
