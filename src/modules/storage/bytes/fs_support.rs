//! Descriptor-anchored directory and ownership checks for the Linux byte adapter.
use crate::modules::services::Error;
use rustix::fs::{self, CWD, Mode, OFlags};
use std::{
    fs::File,
    path::{Component, Path},
};
// Walk all path components through retained no-follow descriptors; create only the final component.
pub(super) fn directory(path: &Path, create: bool) -> Result<File, Error> {
    tracing::trace!("Anchoring a no-follow Storage directory path");
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
pub(super) fn protected(file: &File, directory: bool) -> Result<(), Error> {
    tracing::trace!("Validating Storage store ownership and permissions");
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
