//! Atomic metadata snapshots share the protected byte owner's directory and exclusive writer lifetime.
use super::{Store, fs_support, io};
use crate::modules::{services::Error, storage::contract::Persistence};
use rustix::fs::{self, AtFlags, Mode, OFlags};
use std::{fs::File, io::Read};

impl Persistence for Store {
    // Recover only installed metadata, discard uncommitted staging and bound allocation before reading.
    fn load(&self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, Error> {
        tracing::trace!("Loading bounded installed Storage metadata");
        let read_bound = maximum_bytes.checked_add(1).ok_or(Error::Limit)?;
        cleanup(self)?;
        let file = match fs::openat(
            &self.0.file,
            "metadata.bin",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(fd) => File::from(fd),
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(_) => return Err(Error::Storage),
        };
        fs_support::protected(&file, false)?;
        if file.metadata().map_err(|_| Error::Storage)?.len() > maximum_bytes as u64 {
            return Err(Error::Limit);
        }
        let mut bytes = Vec::new();
        file.take(read_bound as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Storage)?;
        if bytes.len() > maximum_bytes {
            return Err(Error::Limit);
        }
        self.0.file.sync_all().map_err(|_| Error::Unknown)?;
        Ok(Some(bytes))
    }
    // Replace the complete metadata image only after syncing its contents, then sync the rename directory.
    fn replace(&self, bytes: &[u8]) -> Result<(), Error> {
        tracing::trace!("Replacing complete durable Storage metadata");
        let mut file = File::from(
            fs::openat(
                &self.0.file,
                "metadata.pending",
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|_| Error::Storage)?,
        );
        let mut installed = false;
        let result = (|| {
            fs_support::protected(&file, false)?;
            cut(self, 1)?;
            io::write_exact(&mut file, bytes)?;
            cut(self, 2)?;
            file.sync_all().map_err(|_| Error::Storage)?;
            cut(self, 3)?;
            fs::renameat(
                &self.0.file,
                "metadata.pending",
                &self.0.file,
                "metadata.bin",
            )
            .map_err(|_| Error::Storage)?;
            installed = true;
            cut(self, 4)?;
            self.0.file.sync_all().map_err(|_| Error::Unknown)?;
            cut(self, 5)?;
            Ok(())
        })();
        if !installed {
            let _ = fs::unlinkat(&self.0.file, "metadata.pending", AtFlags::empty());
        }
        result
    }
}
// A pending snapshot was never acknowledged as installed; only the current file has recovery authority.
fn cleanup(store: &Store) -> Result<(), Error> {
    tracing::trace!("Discarding uninstalled Storage metadata staging");
    let file = match fs::openat(
        &store.0.file,
        "metadata.pending",
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(rustix::io::Errno::NOENT) => return Ok(()),
        Err(_) => return Err(Error::Storage),
    };
    fs_support::protected(&file, false)?;
    fs::unlinkat(&store.0.file, "metadata.pending", AtFlags::empty())
        .map_err(|_| Error::Storage)?;
    store.0.file.sync_all().map_err(|_| Error::Unknown)
}
// Private test cuts never expose fault selection through production configuration or peer input.
fn cut(store: &Store, point: u8) -> Result<(), Error> {
    tracing::trace!("Checking Storage metadata durability progress");
    #[cfg(test)]
    if store
        .0
        .snapshot_fault
        .load(std::sync::atomic::Ordering::Relaxed)
        == point
    {
        return Err(if point >= 4 {
            Error::Unknown
        } else {
            Error::Storage
        });
    }
    let _ = (store, point);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::storage::bytes::tests::path;
    // Reopening acknowledges complete old/new metadata at every write, sync and rename cut.
    #[test]
    fn snapshot_cut_matrix_and_reopen() {
        for point in 1..=5 {
            let p = path();
            let store = Store::open(&p.0, true).unwrap();
            assert_eq!(store.load(100), Ok(None));
            store.replace(b"old").unwrap();
            store
                .0
                .snapshot_fault
                .store(point, std::sync::atomic::Ordering::Relaxed);
            assert_eq!(
                store.replace(b"new"),
                Err(if point >= 4 {
                    Error::Unknown
                } else {
                    Error::Storage
                })
            );
            drop(store);
            let store = Store::open(&p.0, false).unwrap();
            assert_eq!(
                store.load(100),
                Ok(Some(if point >= 4 {
                    b"new".to_vec()
                } else {
                    b"old".to_vec()
                }))
            );
            assert_eq!(store.load(2), Err(Error::Limit));
            assert_eq!(store.load(usize::MAX), Err(Error::Limit));
        }
    }
    // Owner recovery never promotes a pending image and refuses unsafe metadata entries.
    #[test]
    fn orphan_and_symlink_metadata() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let p = path();
        let store = Store::open(&p.0, true).unwrap();
        store.replace(b"current").unwrap();
        let mut orphan = File::from(
            fs::openat(
                &store.0.file,
                "metadata.pending",
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL,
                Mode::RUSR | Mode::WUSR,
            )
            .unwrap(),
        );
        io::write_exact(&mut orphan, b"uncommitted").unwrap();
        drop(orphan);
        assert_eq!(store.load(100), Ok(Some(b"current".to_vec())));
        assert!(!p.0.join("metadata.pending").exists());
        symlink(p.0.join("metadata.bin"), p.0.join("metadata.pending")).unwrap();
        assert_eq!(store.load(100), Err(Error::Storage));
        std::fs::remove_file(p.0.join("metadata.pending")).unwrap();
        std::fs::set_permissions(
            p.0.join("metadata.bin"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert_eq!(store.load(100), Err(Error::Denied));
    }
}
