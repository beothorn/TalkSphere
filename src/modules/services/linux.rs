//! Linux descriptor-anchored, owner-only durable Services snapshots; separate from Identity storage.
use super::{
    Error,
    contract::Limits,
    persistence::{Persistence, Record},
    record_codec,
};
use rustix::fs::{self, AtFlags, CWD, FlockOperation, Mode, OFlags};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Component, Path},
};

/// Locked protected store. Protection excludes other users, not the same account or root.
pub struct FileStore {
    directory: File,
    _lock: File,
    limits: Limits,
    #[cfg(test)]
    fault: Option<u8>,
}
impl FileStore {
    /// Open an existing directory or explicitly create only the final directory; retain a no-follow directory anchor and exclusive lock.
    /// # Errors
    /// Rejects unsafe paths, insecure ownership/modes, lock contention and unavailable storage.
    pub fn open(path: &Path, limits: Limits, create: bool) -> Result<Self, Error> {
        tracing::trace!("Opening a protected Services store");
        limits.validate()?;
        let directory = directory(path, create)?;
        protected(&directory, true)?;
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
            .map_err(|_| Error::Storage)?,
        );
        protected(&lock, false)?;
        fs::flock(&lock, FlockOperation::NonBlockingLockExclusive).map_err(|_| Error::Denied)?;
        // Uncommitted staging never becomes the current snapshot, including after interrupted writes.
        match fs::openat(
            &directory,
            "services.pending",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(fd) => {
                protected(&File::from(fd), false)?;
                fs::unlinkat(&directory, "services.pending", AtFlags::empty())
                    .map_err(|_| Error::Storage)?;
                directory.sync_all().map_err(|_| Error::Unknown)?;
            }
            Err(rustix::io::Errno::NOENT) => {}
            Err(_) => return Err(Error::Storage),
        }
        Ok(Self {
            directory,
            _lock: lock,
            limits,
            #[cfg(test)]
            fault: None,
        })
    }
    // Test-only cut points bracket durability; no production configuration selects a fault.
    fn cut(&self, point: u8) -> Result<(), Error> {
        tracing::trace!("Checking Services durable commit progress");
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
impl Persistence for FileStore {
    // Load a complete bounded current record and synchronize its directory before recovering uncertain state.
    fn load(&mut self) -> Result<Option<Record>, Error> {
        tracing::trace!("Loading a protected durable Services snapshot");
        let file = match fs::openat(
            &self.directory,
            "services.bin",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(fd) => File::from(fd),
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(_) => return Err(Error::Storage),
        };
        protected(&file, false)?;
        let maximum = record_codec::maximum(self.limits)?;
        if file.metadata().map_err(|_| Error::Storage)?.len() > maximum as u64 {
            return Err(Error::Limit);
        }
        let mut bytes = Vec::new();
        file.take(maximum as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Storage)?;
        let record = record_codec::decode(&bytes, self.limits)?;
        self.directory.sync_all().map_err(|_| Error::Unknown)?;
        Ok(Some(record))
    }
    // Acknowledge only after temporary-file sync, atomic rename and directory sync.
    fn commit(&mut self, record: &Record) -> Result<(), Error> {
        tracing::trace!("Committing a complete protected Services snapshot");
        let bytes = record_codec::encode(record, self.limits)?;
        let mut file = File::from(
            fs::openat(
                &self.directory,
                "services.pending",
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|_| Error::Storage)?,
        );
        let mut installed = false;
        let result = (|| {
            protected(&file, false)?;
            self.cut(1)?;
            file.write_all(&bytes).map_err(|_| Error::Storage)?;
            self.cut(2)?;
            file.sync_all().map_err(|_| Error::Storage)?;
            self.cut(3)?;
            fs::renameat(
                &self.directory,
                "services.pending",
                &self.directory,
                "services.bin",
            )
            .map_err(|_| Error::Storage)?;
            installed = true;
            self.cut(4)?;
            self.directory.sync_all().map_err(|_| Error::Unknown)?;
            self.cut(5)?;
            Ok(())
        })();
        if !installed {
            let _ = fs::unlinkat(&self.directory, "services.pending", AtFlags::empty());
        }
        result
    }
}
// Walk all path components through retained no-follow descriptors; create only the final component.
fn directory(path: &Path, create: bool) -> Result<File, Error> {
    tracing::trace!("Anchoring a no-follow Services directory path");
    let components: Vec<_> = path.components().collect();
    if components.is_empty() {
        return Err(Error::Invalid);
    }
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut dir = File::from(
        fs::openat(
            CWD,
            if path.is_absolute() { "/" } else { "." },
            flags,
            Mode::empty(),
        )
        .map_err(|_| Error::Storage)?,
    );
    for (i, component) in components.iter().enumerate() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                let fd = match fs::openat(&dir, *name, flags, Mode::empty()) {
                    Ok(fd) => fd,
                    Err(rustix::io::Errno::NOENT) if create && i + 1 == components.len() => {
                        match fs::mkdirat(&dir, *name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
                            Ok(()) => {}
                            Err(rustix::io::Errno::EXIST) => {}
                            Err(_) => return Err(Error::Storage),
                        }
                        dir.sync_all().map_err(|_| Error::Unknown)?;
                        fs::openat(&dir, *name, flags, Mode::empty()).map_err(|_| Error::Storage)?
                    }
                    Err(_) => return Err(Error::Storage),
                };
                dir = File::from(fd);
            }
            _ => return Err(Error::Invalid),
        }
    }
    Ok(dir)
}
// Verify metadata on the opened descriptor, including link count, instead of racing a path stat.
fn protected(file: &File, directory: bool) -> Result<(), Error> {
    tracing::trace!("Validating Services store ownership and permissions");
    let m = fs::fstat(file).map_err(|_| Error::Storage)?;
    let mode = if directory { 0o040700 } else { 0o100600 };
    if m.st_uid != rustix::process::geteuid().as_raw()
        || m.st_mode & 0o177777 != mode
        || (!directory && m.st_nlink != 1)
    {
        return Err(Error::Denied);
    }
    Ok(())
}
#[cfg(test)]
#[path = "tests/linux.rs"]
mod tests;
