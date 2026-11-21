//! Internal descriptor-safe physical reclamation waits for all immutable object pins and acknowledges directory durability.
use super::{Store, fs_support, name};
use crate::modules::services::Error;
use rustix::fs::{self, AtFlags, Mode, OFlags};
use std::{
    collections::BTreeMap,
    fs::File,
    sync::{Arc, Weak},
};

// Share one lifetime marker per live object; remove dead weak entries so historical object opens do not grow the registry.
pub(super) fn pin(pins: &mut BTreeMap<[u8; 32], Weak<()>>, id: [u8; 32]) -> Arc<()> {
    tracing::trace!("Retaining immutable Storage object lifetime for safe reclamation");
    pins.retain(|_, weak| weak.strong_count() > 0);
    if let Some(pin) = pins.get(&id).and_then(Weak::upgrade) {
        return pin;
    }
    let pin = Arc::new(());
    pins.insert(id, Arc::downgrade(&pin));
    pin
}
impl Store {
    // Remove provisional names only after the trusted owner has a durable abort for this exact ID.
    pub(crate) fn discard_upload(&self, id: [u8; 32]) -> Result<(), Error> {
        tracing::trace!("Discarding provisional upload after the original durable abort");
        let path = name(id, true);
        match fs::openat(
            &self.0.file,
            &path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(file) => {
                fs_support::protected(&File::from(file), false)?;
                fs::unlinkat(&self.0.file, path, AtFlags::empty()).map_err(|_| Error::Storage)?;
            }
            Err(rustix::io::Errno::NOENT) => {}
            Err(_) => return Err(Error::Storage),
        }
        self.0.file.sync_all().map_err(|_| Error::Unknown)
    }

    // Include descriptor-only bytes in accounting even when an external failure removed their directory name.
    pub(crate) fn has_pins(&self, id: [u8; 32]) -> Result<bool, Error> {
        tracing::trace!("Checking retained Storage object lifetimes before reclamation");
        let pins = self.0.pins.lock().map_err(|_| Error::Unavailable)?;
        Ok(pins.get(&id).is_some_and(|p| p.strong_count() > 0))
    }
    // Only the trusted owner calls after establishing terminal state and no conflicting pending effects.
    // False defers pinned bytes; true confirms durable absence, including recovery after a prior lost unlink reply.
    pub(crate) fn reclaim(&self, id: [u8; 32]) -> Result<bool, Error> {
        tracing::trace!("Reclaiming terminal Storage bytes only after immutable pins are released");
        let mut pins = self.0.pins.lock().map_err(|_| Error::Unavailable)?;
        if pins.get(&id).is_some_and(|p| p.strong_count() > 0) {
            return Ok(false);
        }
        let path = name(id, false);
        match fs::openat(
            &self.0.file,
            &path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(fd) => {
                fs_support::protected(&File::from(fd), false)?;
                self.reclaim_cut(1)?;
                fs::unlinkat(&self.0.file, path, AtFlags::empty()).map_err(|_| Error::Storage)?;
            }
            Err(rustix::io::Errno::NOENT) => {}
            Err(_) => return Err(Error::Storage),
        }
        self.reclaim_cut(2)?;
        self.0.file.sync_all().map_err(|_| Error::Unknown)?;
        pins.remove(&id);
        Ok(true)
    }
    // Deterministic unit-only fault points distinguish confirmed failed deletion from lost post-unlink durability replies.
    fn reclaim_cut(&self, point: u8) -> Result<(), Error> {
        tracing::trace!("Checking Storage reclamation durability progress");
        #[cfg(test)]
        if self
            .0
            .reclaim_fault
            .load(std::sync::atomic::Ordering::SeqCst)
            == point
        {
            return Err(if point == 1 {
                Error::Storage
            } else {
                Error::Unknown
            });
        }
        let _ = point;
        Ok(())
    }
    // Unit scenarios use actual protected files while choosing an exact cleanup failure boundary.
    #[cfg(test)]
    pub(crate) fn fail_reclaim(&self, point: u8) {
        self.0
            .reclaim_fault
            .store(point, std::sync::atomic::Ordering::SeqCst);
    }
}
