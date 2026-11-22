//! Durable, bounded per-peer observations for the node.
//!
//! Callers: node runtime and policy decisions. The table itself lives in
//! [`crate::composition::policy::Observations`]; this file adds protected atomic persistence.
//! Recording only marks the table dirty and [`FileObservations::flush`] writes it, so a flood
//! of events never rewrites the file per event. Observations are advisory: a crash loses at
//! most the events since the last flush, and bootstrap grants are flushed immediately by callers
//! so a grant is never forgotten.
use crate::composition::policy::{Kind, Observations, Summary};
use crate::modules::identity::api::IdentityId;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Largest accepted observation file.
pub const MAXIMUM_FILE_BYTES: u64 = 2 * 1_048_576;

/// Failure to read or write the observation file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    /// The file is unreadable, oversized or malformed.
    Corrupt,
    /// The file could not be written; the table stays dirty so a later flush retries.
    Write,
}

/// File-backed observation table; one writer per node root.
pub struct FileObservations {
    path: PathBuf,
    state: Mutex<State>,
}
struct State {
    table: Observations,
    dirty: bool,
}

impl FileObservations {
    /// Open the table at `path`, or start empty when the file is absent.
    /// # Errors
    /// A present file that is oversized, unreadable or malformed is `Corrupt`; nothing is replaced.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        tracing::trace!("Opening the peer observation store");
        let table = match std::fs::metadata(path) {
            Ok(meta) if meta.len() > MAXIMUM_FILE_BYTES => return Err(StoreError::Corrupt),
            Ok(_) => {
                let bytes = std::fs::read(path).map_err(|_| StoreError::Corrupt)?;
                let value = serde_json::from_slice(&bytes).map_err(|_| StoreError::Corrupt)?;
                Observations::from_json(&value).map_err(|_| StoreError::Corrupt)?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Observations::new(),
            Err(_) => return Err(StoreError::Corrupt),
        };
        Ok(Self {
            path: path.to_path_buf(),
            state: Mutex::new(State {
                table,
                dirty: false,
            }),
        })
    }

    /// Record one observation; it becomes durable at the next flush.
    pub fn record(&self, peer: IdentityId, kind: Kind, now: u64) {
        tracing::trace!("Recording a durable peer observation");
        self.with(|s| s.table.record(peer, kind, now));
    }

    /// Record a bootstrap grant and write it before returning, so it is never forgotten.
    /// # Errors
    /// `Write` when the file cannot be written; the grant stays in memory and dirty.
    pub fn record_bootstrap(
        &self,
        peer: IdentityId,
        credits: u64,
        now: u64,
    ) -> Result<(), StoreError> {
        tracing::trace!("Recording a durable bootstrap grant");
        self.with(|s| s.table.record_bootstrap(peer, credits, now));
        self.flush()
    }

    /// What is known about `peer`.
    pub fn summary(&self, peer: IdentityId) -> Summary {
        tracing::trace!("Reading a durable peer summary");
        self.read(|t| t.summary(peer))
    }

    /// Total bootstrap credits recorded.
    pub fn bootstrap_total(&self) -> u64 {
        tracing::trace!("Reading the durable bootstrap total");
        self.read(Observations::bootstrap_total)
    }

    /// Drop idle entries; returns how many were dropped.
    pub fn age_out(&self, now: u64) -> usize {
        tracing::trace!("Ageing the durable peer observations");
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let dropped = state.table.age_out(now);
        state.dirty |= dropped > 0;
        dropped
    }

    /// Write the table atomically when it changed since the last write.
    /// # Errors
    /// `Write` when the temporary file or rename fails; the table stays dirty.
    pub fn flush(&self) -> Result<(), StoreError> {
        tracing::trace!("Flushing the peer observation store");
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if !state.dirty {
            return Ok(());
        }
        self.persist(&state.table)?;
        state.dirty = false;
        Ok(())
    }

    // Run `f` on the table and mark it dirty; a poisoned lock still yields the data.
    fn with<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.dirty = true;
        f(&mut state)
    }

    // Read the table without marking it dirty.
    fn read<T>(&self, f: impl FnOnce(&Observations) -> T) -> T {
        f(&self.state.lock().unwrap_or_else(|e| e.into_inner()).table)
    }

    // Replace the file atomically: write a private temporary, flush it, then rename over the file.
    fn persist(&self, table: &Observations) -> Result<(), StoreError> {
        use std::os::unix::fs::OpenOptionsExt;
        let bytes = serde_json::to_vec(&table.to_json()).map_err(|_| StoreError::Write)?;
        let temporary = self.path.with_extension("tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| StoreError::Write)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| StoreError::Write)?;
        std::fs::rename(&temporary, &self.path).map_err(|_| StoreError::Write)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A fresh private directory per test.
    fn dir() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "ts-observations-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    // Flushed observations survive a reopen; unflushed ones are advisory and may be lost.
    #[test]
    fn flushed_observations_persist_across_reopen() {
        let path = dir().join("observations.json");
        let me = IdentityId([1; 32]);
        let store = FileObservations::open(&path).unwrap();
        store.record(me, Kind::Connected, 10);
        store.flush().unwrap();
        store.record(me, Kind::Connected, 11);
        let reopened = FileObservations::open(&path).unwrap();
        assert_eq!(reopened.summary(me).connections, 1);
        store.flush().unwrap();
        assert_eq!(
            FileObservations::open(&path)
                .unwrap()
                .summary(me)
                .connections,
            2
        );
    }

    // A bootstrap grant is durable before the call returns, so a crash cannot allow a second grant.
    #[test]
    fn bootstrap_grants_are_durable_immediately() {
        let path = dir().join("observations.json");
        let me = IdentityId([2; 32]);
        FileObservations::open(&path)
            .unwrap()
            .record_bootstrap(me, 25, 5)
            .unwrap();
        let reopened = FileObservations::open(&path).unwrap();
        assert_eq!(reopened.summary(me).bootstrap_credits, 25);
        assert_eq!(reopened.bootstrap_total(), 25);
    }

    // Corrupt, oversized and unreadable files are refused and never replaced by an empty table.
    #[test]
    fn corrupt_files_are_refused_and_left_alone() {
        let root = dir();
        let path = root.join("observations.json");
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(
            FileObservations::open(&path).err(),
            Some(StoreError::Corrupt)
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"not json");
        std::fs::write(&path, vec![b' '; MAXIMUM_FILE_BYTES as usize + 1]).unwrap();
        assert_eq!(
            FileObservations::open(&path).err(),
            Some(StoreError::Corrupt)
        );
        assert_eq!(
            FileObservations::open(&root).err(),
            Some(StoreError::Corrupt)
        );
    }

    // A failed write reports the error and keeps the table dirty so a later flush can retry.
    #[test]
    fn write_failure_keeps_the_table_dirty() {
        let root = dir();
        let path = root.join("sub").join("observations.json");
        std::fs::create_dir(root.join("sub")).unwrap();
        let store = FileObservations::open(&path).unwrap();
        store.record(IdentityId([3; 32]), Kind::Failed, 1);
        std::fs::remove_dir_all(root.join("sub")).unwrap();
        assert_eq!(store.flush(), Err(StoreError::Write));
        std::fs::create_dir(root.join("sub")).unwrap();
        store.flush().unwrap();
        assert_eq!(
            FileObservations::open(&path)
                .unwrap()
                .summary(IdentityId([3; 32]))
                .failures,
            1
        );
    }

    // Ageing marks the table dirty only when something was dropped, and the file is owner-only.
    #[test]
    fn ageing_persists_and_the_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let path = dir().join("observations.json");
        let store = FileObservations::open(&path).unwrap();
        store.record(IdentityId([4; 32]), Kind::Connected, 0);
        store.flush().unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            store.age_out(crate::composition::policy::observations::IDLE_SECONDS + 1),
            1
        );
        store.flush().unwrap();
        assert_eq!(
            FileObservations::open(&path)
                .unwrap()
                .summary(IdentityId([4; 32])),
            Summary::default()
        );
    }
}
