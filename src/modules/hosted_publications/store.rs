//! Durable, bounded placement storage with atomic updates.
//!
//! Callers: the lifecycle rules and the provider engine. The store is the module's own local persistence, independent of the paid
//! Storage service and of Messaging. A placement is inserted once and never replaced; every change goes through [`Store::update`], which
//! applies a closure to the record under a lock and persists it atomically (a failed write changes nothing and the previous record
//! stays). Native stores write owner-only files (content first, then the record) and rebuild an index of records when opened; volatile
//! stores keep everything in memory with injectable write failures for tests. The duplicate-publication and same-sequence rules are
//! enforced inside [`Store::insert`] under the same lock, so two racing stores cannot both succeed. All sizes and counts are bounded.
use super::{
    error::Error,
    ids::{ContentHash, PlacementId, PublicationId, RemovalId, Signature},
    metadata::{Kind, Metadata, Visibility},
    placement::{Placement, Record, Removal, State},
};
use crate::modules::{
    identity::{api::IdentityId, text},
    messaging::file_io::{create_dir, hex, unhex, write_atomic},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Finite bounds of one store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Most placements held (any state until reclaimed).
    pub placements: usize,
    /// Most content bytes held for active placements.
    pub content_bytes: u64,
}

/// Largest accepted record file: a maximum audience of 65535 readers in text form plus the fixed members.
const MAXIMUM_RECORD_BYTES: u64 = 4 * 1024 * 1024;

// Where records and contents live.
enum Backing {
    Disk(PathBuf),
    Memory {
        contents: BTreeMap<PlacementId, Vec<u8>>,
        fail_writes: bool,
    },
}

struct Inner {
    index: BTreeMap<PlacementId, Record>,
    backing: Backing,
}

/// The placement store.
pub struct Store {
    limits: Limits,
    state: Mutex<Inner>,
}

impl Store {
    /// A volatile store for tests and simulations.
    pub fn volatile(limits: Limits) -> Self {
        tracing::trace!("Creating a volatile hosted placement store");
        Self {
            limits,
            state: Mutex::new(Inner {
                index: BTreeMap::new(),
                backing: Backing::Memory {
                    contents: BTreeMap::new(),
                    fail_writes: false,
                },
            }),
        }
    }

    /// Open (creating when absent) a native store in `dir`, loading every record.
    /// # Errors
    /// `Storage` when the directory cannot be created or a record file is unreadable, oversized, malformed or inconsistent with its
    /// content; nothing is repaired or replaced.
    pub fn open(dir: &Path, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Opening the native hosted placement store");
        create_dir(dir)?;
        let index = native::load(dir, limits)?;
        Ok(Self {
            limits,
            state: Mutex::new(Inner {
                index,
                backing: Backing::Disk(dir.to_path_buf()),
            }),
        })
    }

    /// Make later writes of a volatile store fail (or succeed again), to test write-failure paths. No effect on native stores.
    pub fn fail_writes(&self, fail: bool) {
        if let Backing::Memory { fail_writes, .. } = &mut self.lock().backing {
            *fail_writes = fail;
        }
    }

    /// Insert a new placement durably at time `now`; never replaces an existing one.
    ///
    /// Besides the placement ID and the finite bounds, the specification's host rules are enforced atomically here: a publication ID
    /// that already has an active placement of this publisher is a `Conflict` (the publisher renews instead), and so is a page or feed
    /// post whose sequence equals that of an active placement of the same publisher and kind with another publication ID.
    /// # Errors
    /// `Conflict` as above, `Limit` when a finite bound would be exceeded, `Storage` when the write fails (nothing is recorded).
    pub fn insert(&self, placement: &Placement, now: u64) -> Result<(), Error> {
        tracing::trace!("Inserting a hosted placement");
        let mut state = self.lock();
        let id = placement.record.id;
        let metadata = &placement.record.metadata;
        if state.index.contains_key(&id) {
            return Err(Error::Conflict);
        }
        if conflicts_with(&state.index, metadata, now) {
            return Err(Error::Conflict);
        }
        if state.index.len() >= self.limits.placements
            || self
                .active_bytes(&state)
                .saturating_add(placement.content.len() as u64)
                > self.limits.content_bytes
        {
            return Err(Error::Limit);
        }
        write_content(&mut state.backing, id, &placement.content)?;
        if let Err(error) = write_record(&mut state.backing, &placement.record) {
            discard_content(&mut state.backing, id);
            return Err(error);
        }
        state.index.insert(id, placement.record.clone());
        Ok(())
    }

    /// Whether storing a publication with this metadata at `now` would break the host rules: an active placement of the same publisher
    /// with the same publication ID, or (for pages and feed posts) the same kind and sequence with another publication ID.
    pub fn conflicts(&self, metadata: &Metadata, now: u64) -> bool {
        tracing::trace!("Checking the hosted store rules for a publication");
        conflicts_with(&self.lock().index, metadata, now)
    }

    /// Whether one more placement with `content_len` content bytes would fit the finite bounds right now.
    pub fn has_room(&self, content_len: u64) -> bool {
        tracing::trace!("Checking hosted placement store capacity");
        let state = self.lock();
        state.index.len() < self.limits.placements
            && self.active_bytes(&state).saturating_add(content_len) <= self.limits.content_bytes
    }

    /// The record of a placement, without its content.
    pub fn record(&self, id: PlacementId) -> Option<Record> {
        tracing::trace!("Reading a hosted placement record");
        self.lock().index.get(&id).cloned()
    }

    /// A placement with its content.
    /// # Errors
    /// `Storage` when the content is missing or the wrong size (never returned partly).
    pub fn get(&self, id: PlacementId) -> Result<Option<Placement>, Error> {
        tracing::trace!("Reading a hosted placement");
        let state = self.lock();
        let Some(record) = state.index.get(&id) else {
            return Ok(None);
        };
        let content = read_content(&state.backing, id).ok_or(Error::Storage)?;
        if content.len() as u64 != record.metadata.content_size {
            return Err(Error::Storage);
        }
        Ok(Some(Placement {
            record: record.clone(),
            content,
        }))
    }

    /// Apply `change` to a record under the store lock and persist the result atomically. The closure may refuse; a refusal or a failed
    /// write leaves the previous record in place. Placement ID, publication metadata and signature never change.
    /// # Errors
    /// `NotAvailable` for an unknown placement, the closure's own error, or `Storage` when the write fails.
    pub fn update(
        &self,
        id: PlacementId,
        change: &mut dyn FnMut(&mut Record) -> Result<(), Error>,
    ) -> Result<Record, Error> {
        tracing::trace!("Updating a hosted placement record");
        let mut state = self.lock();
        let before = state.index.get(&id).cloned().ok_or(Error::NotAvailable)?;
        let mut after = before.clone();
        change(&mut after)?;
        after.id = before.id;
        after.metadata = before.metadata.clone();
        after.publisher_public_key = before.publisher_public_key;
        after.publisher_signature = before.publisher_signature;
        write_record(&mut state.backing, &after)?;
        state.index.insert(id, after.clone());
        Ok(after)
    }

    /// The active placement of one publication of one publisher at `now`, if any.
    pub fn active_publication(
        &self,
        publisher: IdentityId,
        publication: PublicationId,
        now: u64,
    ) -> Option<Record> {
        tracing::trace!("Finding the active placement of a publication");
        self.lock()
            .index
            .values()
            .find(|r| {
                r.active_at(now)
                    && r.metadata.publisher_id == publisher
                    && r.metadata.publication_id == publication
            })
            .cloned()
    }

    /// Active placements of one publisher and kind at `now`, ordered newest first by `(sequence, publicationId)` descending.
    pub fn active_of_kind(&self, publisher: IdentityId, kind: Kind, now: u64) -> Vec<Record> {
        tracing::trace!("Listing active placements of a publisher and kind");
        let mut found: Vec<Record> = self
            .lock()
            .index
            .values()
            .filter(|r| {
                r.active_at(now) && r.metadata.publisher_id == publisher && r.metadata.kind == kind
            })
            .cloned()
            .collect();
        found.sort_by(|a, b| {
            (b.metadata.sequence, b.metadata.publication_id)
                .cmp(&(a.metadata.sequence, a.metadata.publication_id))
        });
        found
    }

    /// How many active placements one publisher holds at `now`, for quotas.
    pub fn active_count(&self, publisher: IdentityId, now: u64) -> usize {
        tracing::trace!("Counting active placements of a publisher");
        self.lock()
            .index
            .values()
            .filter(|r| r.active_at(now) && r.metadata.publisher_id == publisher)
            .count()
    }

    /// Every record, in identifier order.
    pub fn records(&self) -> Vec<Record> {
        tracing::trace!("Listing every hosted placement record");
        self.lock().index.values().cloned().collect()
    }

    /// Release the content bytes of a placement that no longer owes them; the record stays.
    /// # Errors
    /// `NotAvailable` for an unknown placement.
    pub fn release_content(&self, id: PlacementId) -> Result<(), Error> {
        tracing::trace!("Releasing hosted placement content");
        let mut state = self.lock();
        if !state.index.contains_key(&id) {
            return Err(Error::NotAvailable);
        }
        discard_content(&mut state.backing, id);
        Ok(())
    }

    /// Forget a placement that no longer owes anything: its content first, then its record, so a crash in between leaves a valid record
    /// that simply has no content. Discarding an unknown placement is a no-op.
    /// # Errors
    /// `Storage` when the record cannot be removed (the placement then stays recorded).
    pub fn discard(&self, id: PlacementId) -> Result<(), Error> {
        tracing::trace!("Discarding a hosted placement");
        let mut state = self.lock();
        if !state.index.contains_key(&id) {
            return Ok(());
        }
        discard_content(&mut state.backing, id);
        remove_record(&mut state.backing, id)?;
        state.index.remove(&id);
        Ok(())
    }

    // Content bytes of placements that still owe them.
    fn active_bytes(&self, state: &Inner) -> u64 {
        state
            .index
            .values()
            .filter(|r| r.state.is_active())
            .map(|r| r.metadata.content_size)
            .fold(0u64, u64::saturating_add)
    }

    // A poisoned lock still holds a consistent index because every update replaces one whole record.
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// The host rules shared by `insert` and `conflicts`.
fn conflicts_with(index: &BTreeMap<PlacementId, Record>, metadata: &Metadata, now: u64) -> bool {
    index.values().filter(|r| r.active_at(now)).any(|r| {
        r.metadata.publisher_id == metadata.publisher_id
            && (r.metadata.publication_id == metadata.publication_id
                || (metadata.kind.is_listed()
                    && r.metadata.kind == metadata.kind
                    && r.metadata.sequence == metadata.sequence))
    })
}

// Write a content: memory or an owner-only file replaced atomically.
fn write_content(backing: &mut Backing, id: PlacementId, content: &[u8]) -> Result<(), Error> {
    match backing {
        Backing::Memory {
            contents,
            fail_writes,
        } => {
            if *fail_writes {
                return Err(Error::Storage);
            }
            contents.insert(id, content.to_vec());
            Ok(())
        }
        Backing::Disk(dir) => {
            write_atomic(&native::content_path(dir, id), content).map_err(Error::from)
        }
    }
}

// Write a record the same way.
fn write_record(backing: &mut Backing, record: &Record) -> Result<(), Error> {
    match backing {
        Backing::Memory { fail_writes, .. } => {
            if *fail_writes {
                return Err(Error::Storage);
            }
            Ok(())
        }
        Backing::Disk(dir) => {
            let bytes = serde_json::to_vec(&record_json(record)).map_err(|_| Error::Storage)?;
            write_atomic(&native::record_path(dir, record.id), &bytes).map_err(Error::from)
        }
    }
}

// Remove a record: nothing in memory, the owner-only file on disk. A missing file is already removed.
fn remove_record(backing: &mut Backing, id: PlacementId) -> Result<(), Error> {
    match backing {
        Backing::Memory { fail_writes, .. } => {
            if *fail_writes {
                return Err(Error::Storage);
            }
            Ok(())
        }
        Backing::Disk(dir) => match std::fs::remove_file(native::record_path(dir, id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(Error::Storage),
        },
    }
}

fn read_content(backing: &Backing, id: PlacementId) -> Option<Vec<u8>> {
    match backing {
        Backing::Memory { contents, .. } => contents.get(&id).cloned(),
        Backing::Disk(dir) => std::fs::read(native::content_path(dir, id)).ok(),
    }
}

fn discard_content(backing: &mut Backing, id: PlacementId) {
    match backing {
        Backing::Memory { contents, .. } => {
            contents.remove(&id);
        }
        Backing::Disk(dir) => {
            let _ = std::fs::remove_file(native::content_path(dir, id));
        }
    }
}

// JSON form of a record.
fn record_json(r: &Record) -> Value {
    let m = &r.metadata;
    json!({
        "version": 1, "id": hex(&r.id.0), "publicationId": hex(&m.publication_id.0),
        "publisherId": text::format(m.publisher_id), "kind": m.kind.name(),
        "sequence": m.sequence, "publishedAt": m.published_at, "visibility": m.visibility.name(),
        "audience": m.audience.iter().map(|id| text::format(*id)).collect::<Vec<_>>(),
        "contentSize": m.content_size, "contentHash": hex(&m.content_hash.0),
        "publisherPublicKey": hex(&r.publisher_public_key), "publisherSignature": hex(&r.publisher_signature.0),
        "acceptedAt": r.accepted_at, "expiresAt": r.expires_at,
        "state": r.state.name(), "payments": r.payments,
        "removal": r.removal.as_ref().map(|x| json!({
            "removalId": hex(&x.removal_id.0), "releaseCredits": x.release_credits, "price": x.price,
        })),
    })
}

// Parse a stored record strictly; any deviation is corruption.
fn parse_record(value: &Value) -> Option<Record> {
    let o = value.as_object()?;
    if o.len() != 18 || o.get("version")?.as_u64() != Some(1) {
        return None;
    }
    let text_of = |name: &str| o.get(name)?.as_str();
    let number = |name: &str| o.get(name)?.as_u64();
    let removal = match o.get("removal")? {
        Value::Null => None,
        r => {
            let r = r.as_object().filter(|r| r.len() == 3)?;
            Some(Removal {
                removal_id: RemovalId(unhex(r.get("removalId")?.as_str()?)?),
                release_credits: r.get("releaseCredits")?.as_u64()?,
                price: r.get("price")?.as_u64()?,
            })
        }
    };
    let audience = o
        .get("audience")?
        .as_array()?
        .iter()
        .map(|id| text::parse(id.as_str()?).ok())
        .collect::<Option<Vec<_>>>()?;
    Some(Record {
        id: PlacementId(unhex(text_of("id")?)?),
        metadata: Metadata {
            publication_id: PublicationId(unhex(text_of("publicationId")?)?),
            publisher_id: text::parse(text_of("publisherId")?).ok()?,
            kind: Kind::from_name(text_of("kind")?).ok()?,
            sequence: number("sequence")?,
            published_at: number("publishedAt")?,
            visibility: Visibility::from_name(text_of("visibility")?).ok()?,
            audience,
            content_size: number("contentSize")?,
            content_hash: ContentHash(unhex(text_of("contentHash")?)?),
        },
        publisher_public_key: unhex(text_of("publisherPublicKey")?)?,
        publisher_signature: Signature(unhex(text_of("publisherSignature")?)?),
        accepted_at: number("acceptedAt")?,
        expires_at: number("expiresAt")?,
        state: State::parse(text_of("state")?)?,
        payments: number("payments")?,
        removal,
    })
}

// Native file operations: paths and a strict loader; atomic writes live in Messaging's shared `file_io`.
mod native {
    use super::*;

    pub(super) fn record_path(dir: &Path, id: PlacementId) -> PathBuf {
        dir.join(format!("{}.json", hex(&id.0)))
    }
    pub(super) fn content_path(dir: &Path, id: PlacementId) -> PathBuf {
        dir.join(format!("{}.content", hex(&id.0)))
    }
    // Load every record, verifying that each active record's content exists with the declared size.
    pub(super) fn load(dir: &Path, limits: Limits) -> Result<BTreeMap<PlacementId, Record>, Error> {
        let mut index = BTreeMap::new();
        for entry in std::fs::read_dir(dir).map_err(|_| Error::Storage)? {
            let path = entry.map_err(|_| Error::Storage)?.path();
            match path.extension().and_then(|e| e.to_str()) {
                Some("json") => {}
                // A leftover temporary file is an interrupted write; contents are checked against their records.
                Some("tmp") => {
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
                _ => continue,
            }
            let meta = std::fs::metadata(&path).map_err(|_| Error::Storage)?;
            if meta.len() > MAXIMUM_RECORD_BYTES {
                return Err(Error::Storage);
            }
            let value: Value =
                serde_json::from_slice(&std::fs::read(&path).map_err(|_| Error::Storage)?)
                    .map_err(|_| Error::Storage)?;
            let record = parse_record(&value).ok_or(Error::Storage)?;
            if path != record_path(dir, record.id) {
                return Err(Error::Storage);
            }
            if record.state.is_active() {
                let length = std::fs::metadata(content_path(dir, record.id))
                    .map_err(|_| Error::Storage)?
                    .len();
                if length != record.metadata.content_size {
                    return Err(Error::Storage);
                }
            }
            if index.insert(record.id, record).is_some() || index.len() > limits.placements {
                return Err(Error::Storage);
            }
        }
        Ok(index)
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
pub(crate) mod tests;
