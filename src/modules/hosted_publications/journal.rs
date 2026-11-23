//! Durable participant journal: what each accepted hosted publication operation's effect is and how it ended.
//!
//! Callers: the participant. One entry per Services operation key records `Prepared` (the exact effect and the
//! token that authorizes its commit), `Committed` (the outcome, retained so a lost reply can be answered from the
//! original result) or `Aborted`. Entries are replaced atomically, bounded in number (the oldest finished entry is
//! evicted when full; prepared entries are never evicted), and an unreadable entry is refused, never guessed at.
use super::{
    effect::{Effect, Outcome},
    error::Error,
};
use crate::modules::messaging::file_io::{create_dir, hex, unhex, unhex_vec, write_atomic};
use crate::modules::services::contract::OperationKey;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
    sync::Mutex,
};

/// How one operation's effect stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// The exact effect is durably prepared; only this token may commit it.
    Prepared {
        /// The effect.
        effect: Effect,
        /// The commit token.
        token: [u8; 32],
    },
    /// The effect was applied exactly once; the outcome is retained.
    Committed(Outcome),
    /// The coordinator aborted before commitment; nothing was applied.
    Aborted,
}

enum Backing {
    Disk(PathBuf),
    Memory { fail_writes: bool },
}

struct Inner {
    entries: BTreeMap<OperationKey, Entry>,
    /// Finished entries, oldest first, for eviction.
    finished: VecDeque<OperationKey>,
    backing: Backing,
}

/// The journal.
pub struct Journal {
    limit: usize,
    inner: Mutex<Inner>,
}

impl Journal {
    /// A volatile journal for tests; `fail_writes` injects write failures.
    pub fn volatile(limit: usize) -> Self {
        tracing::trace!("Creating a volatile participant journal");
        Self {
            limit,
            inner: Mutex::new(Inner {
                entries: BTreeMap::new(),
                finished: VecDeque::new(),
                backing: Backing::Memory { fail_writes: false },
            }),
        }
    }

    /// Open (creating when absent) a native journal in `dir`, loading every entry.
    /// # Errors
    /// `Storage` when the directory or an entry is unreadable, oversized, malformed or misnamed; nothing is repaired.
    pub fn open(dir: &Path, limit: usize) -> Result<Self, Error> {
        tracing::trace!("Opening the native participant journal");
        create_dir(dir)?;
        let mut entries = BTreeMap::new();
        for item in std::fs::read_dir(dir).map_err(|_| Error::Storage)? {
            let path = item.map_err(|_| Error::Storage)?.path();
            match path.extension().and_then(|e| e.to_str()) {
                Some("tmp") => {
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
                Some("json") => {}
                _ => continue,
            }
            if std::fs::metadata(&path).map_err(|_| Error::Storage)?.len() > MAXIMUM_ENTRY_BYTES {
                return Err(Error::Storage);
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or(Error::Storage)?;
            let key = parse_key(name).ok_or(Error::Storage)?;
            let entry = parse_entry(&std::fs::read(&path).map_err(|_| Error::Storage)?)?;
            entries.insert(key, entry);
        }
        if entries.len() > limit {
            return Err(Error::Storage);
        }
        let finished = entries
            .iter()
            .filter(|(_, e)| !matches!(e, Entry::Prepared { .. }))
            .map(|(k, _)| *k)
            .collect();
        Ok(Self {
            limit,
            inner: Mutex::new(Inner {
                entries,
                finished,
                backing: Backing::Disk(dir.to_path_buf()),
            }),
        })
    }

    /// Make later writes of a volatile journal fail or succeed again.
    pub fn fail_writes(&self, fail: bool) {
        if let Backing::Memory { fail_writes } = &mut self.lock().backing {
            *fail_writes = fail;
        }
    }

    /// The entry of an operation.
    pub fn get(&self, key: OperationKey) -> Option<Entry> {
        tracing::trace!("Reading a participant journal entry");
        self.lock().entries.get(&key).cloned()
    }

    /// Record or replace an entry atomically. A new key beyond the bound evicts the oldest finished entry.
    /// # Errors
    /// `Limit` when the journal is full of prepared entries, `Storage` when the write fails (nothing changes).
    pub fn put(&self, key: OperationKey, entry: Entry) -> Result<(), Error> {
        tracing::trace!("Writing a participant journal entry");
        let mut inner = self.lock();
        let evict = if !inner.entries.contains_key(&key) && inner.entries.len() >= self.limit {
            Some(inner.finished.front().copied().ok_or(Error::Limit)?)
        } else {
            None
        };
        write(&inner.backing, key, &entry)?;
        if let Some(old) = evict {
            inner.entries.remove(&old);
            inner.finished.pop_front();
            if let Backing::Disk(dir) = &inner.backing {
                let _ = std::fs::remove_file(path_of(dir, old));
            }
        }
        if !matches!(entry, Entry::Prepared { .. }) && !inner.finished.contains(&key) {
            inner.finished.push_back(key);
        }
        inner.entries.insert(key, entry);
        Ok(())
    }

    /// Every prepared entry, for recovery.
    pub fn prepared(&self) -> Vec<(OperationKey, Effect, [u8; 32])> {
        tracing::trace!("Listing prepared participant entries");
        self.lock()
            .entries
            .iter()
            .filter_map(|(k, e)| match e {
                Entry::Prepared { effect, token } => Some((*k, effect.clone(), *token)),
                _ => None,
            })
            .collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Largest accepted entry file: a store effect carries a publication of at most 16 KiB with an audience of up to 65535 readers, hex-encoded
/// inside the entry, with its bookkeeping.
const MAXIMUM_ENTRY_BYTES: u64 = 8 * 1024 * 1024;

// File name of a key: requester, connection and stream as lowercase hex.
fn path_of(dir: &Path, key: OperationKey) -> PathBuf {
    dir.join(format!(
        "{}{}{:08x}.json",
        hex(&key.requester.0),
        hex(&key.connection),
        key.stream
    ))
}

fn parse_key(name: &str) -> Option<OperationKey> {
    if name.len() != 64 + 64 + 8 {
        return None;
    }
    Some(OperationKey {
        requester: crate::modules::identity::api::IdentityId(unhex(&name[..64])?),
        connection: unhex(&name[64..128])?,
        stream: u32::from_str_radix(&name[128..], 16)
            .ok()
            .filter(|_| name[128..].bytes().all(|b| !b.is_ascii_uppercase()))?,
    })
}

// Persist one entry in the native backing, or honor an injected failure in memory.
fn write(backing: &Backing, key: OperationKey, entry: &Entry) -> Result<(), Error> {
    match backing {
        Backing::Memory { fail_writes } => {
            if *fail_writes {
                Err(Error::Storage)
            } else {
                Ok(())
            }
        }
        Backing::Disk(dir) => {
            let value = match entry {
                Entry::Prepared { effect, token } => {
                    json!({"state": "prepared", "token": hex(token), "effect": hex(&effect.encode())})
                }
                Entry::Committed(outcome) => {
                    json!({"state": "committed", "outcome": hex(&outcome.encode())})
                }
                Entry::Aborted => json!({"state": "aborted"}),
            };
            write_atomic(
                &path_of(dir, key),
                &serde_json::to_vec(&value).map_err(|_| Error::Storage)?,
            )
            .map_err(Error::from)
        }
    }
}

// Strictly parse one stored entry.
fn parse_entry(bytes: &[u8]) -> Result<Entry, Error> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Error::Storage)?;
    let o = value.as_object().ok_or(Error::Storage)?;
    let text = |n: &str| o.get(n).and_then(Value::as_str);
    Ok(match (text("state").ok_or(Error::Storage)?, o.len()) {
        ("prepared", 3) => Entry::Prepared {
            token: text("token").and_then(unhex::<32>).ok_or(Error::Storage)?,
            effect: Effect::decode(&text("effect").and_then(unhex_vec).ok_or(Error::Storage)?)?,
        },
        ("committed", 2) => Entry::Committed(Outcome::decode(
            &text("outcome").and_then(unhex_vec).ok_or(Error::Storage)?,
        )?),
        ("aborted", 1) => Entry::Aborted,
        _ => return Err(Error::Storage),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{hosted_publications::ids::PlacementId, identity::api::IdentityId};

    fn key(n: u8) -> OperationKey {
        OperationKey {
            requester: IdentityId([n; 32]),
            connection: [n; 32],
            stream: u32::from(n),
        }
    }
    fn dir() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        std::env::temp_dir().join(format!(
            "ts-hosted-journal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    fn prepared() -> Entry {
        Entry::Prepared {
            effect: Effect::Renew {
                placement_id: PlacementId([1; 32]),
                expected_expires_at: 10,
                additional_seconds: 4,
                ceiling_expires_at: 100,
                price: 6,
            },
            token: [9; 32],
        }
    }

    // Entries are replaced in place, and a native journal reopens with identical prepared, committed and aborted entries.
    #[test]
    fn entries_persist_across_reopen() {
        let path = dir();
        let journal = Journal::open(&path, 8).unwrap();
        journal.put(key(1), prepared()).unwrap();
        journal.put(key(2), Entry::Aborted).unwrap();
        journal
            .put(key(3), Entry::Committed(Outcome::Query))
            .unwrap();
        journal
            .put(key(1), Entry::Committed(Outcome::Query))
            .unwrap();
        drop(journal);
        let reopened = Journal::open(&path, 8).unwrap();
        assert_eq!(reopened.get(key(1)), Some(Entry::Committed(Outcome::Query)));
        assert_eq!(reopened.get(key(2)), Some(Entry::Aborted));
        assert_eq!(reopened.get(key(9)), None);
        reopened.put(key(4), prepared()).unwrap();
        assert_eq!(reopened.prepared().len(), 1);
    }

    // At the bound the oldest finished entry is evicted, prepared entries never are, and a journal full of them refuses more.
    #[test]
    fn the_journal_is_bounded() {
        let journal = Journal::volatile(2);
        journal.put(key(1), Entry::Aborted).unwrap();
        journal.put(key(2), prepared()).unwrap();
        journal.put(key(3), prepared()).unwrap();
        assert_eq!(journal.get(key(1)), None);
        assert!(journal.get(key(2)).is_some());
        assert_eq!(journal.put(key(4), prepared()), Err(Error::Limit));
        journal.put(key(2), Entry::Aborted).unwrap();
        journal.put(key(4), prepared()).unwrap();
        assert_eq!(journal.get(key(2)), None);
    }

    // A failed write changes nothing.
    #[test]
    fn failed_writes_change_nothing() {
        let journal = Journal::volatile(4);
        journal.put(key(1), prepared()).unwrap();
        journal.fail_writes(true);
        assert_eq!(journal.put(key(1), Entry::Aborted), Err(Error::Storage));
        assert_eq!(journal.put(key(2), prepared()), Err(Error::Storage));
        assert_eq!(journal.get(key(1)), Some(prepared()));
        assert_eq!(journal.get(key(2)), None);
    }

    // Corrupt, oversized, misnamed and unknown-state files make open fail untouched; leftover temporary files are discarded;
    // files are owner-only.
    #[test]
    fn damaged_journals_are_refused() {
        use std::os::unix::fs::PermissionsExt;
        let path = dir();
        let journal = Journal::open(&path, 8).unwrap();
        journal.put(key(1), prepared()).unwrap();
        drop(journal);
        let file = path_of(&path, key(1));
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let good = std::fs::read(&file).unwrap();
        for bad in [
            b"nonsense".to_vec(),
            br#"{"state":"limbo"}"#.to_vec(),
            br#"{"state":"aborted","extra":1}"#.to_vec(),
            vec![b' '; MAXIMUM_ENTRY_BYTES as usize + 1],
        ] {
            std::fs::write(&file, &bad).unwrap();
            assert_eq!(Journal::open(&path, 8).err(), Some(Error::Storage));
            assert_eq!(std::fs::read(&file).unwrap(), bad);
        }
        std::fs::write(&file, &good).unwrap();
        std::fs::write(path.join("half.tmp"), b"x").unwrap();
        assert!(Journal::open(&path, 8).is_ok());
        assert!(!path.join("half.tmp").exists());
        std::fs::copy(&file, path.join("misnamed.json")).unwrap();
        assert_eq!(Journal::open(&path, 8).err(), Some(Error::Storage));
    }
}
