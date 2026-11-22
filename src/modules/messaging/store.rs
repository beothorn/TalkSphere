//! Durable, bounded placement storage with atomic updates.
//!
//! Callers: the provider engine. The store is the module's own local persistence, independent of the paid
//! Storage service. A placement is inserted once and never replaced; every change goes through [`Store::update`],
//! which applies a closure to the record under a lock and persists it atomically (a failed write changes nothing
//! and the previous record stays). Native stores write owner-only files (payload first, then the record) and
//! rebuild an index of records when opened; volatile stores keep everything in memory with injectable write
//! failures for tests. All sizes and counts are bounded.
use super::{
    error::Error,
    ids::{MessageId, PayloadHash, PlacementId, RemovalId, Signature},
    metadata::Metadata,
    operation::Receipt,
    placement::{Placement, Record, Removal},
    state::{Encoding, State},
};
use crate::modules::identity::{api::IdentityId, text};
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
    /// Most payload bytes held for active placements.
    pub payload_bytes: u64,
}

// Where records and payloads live.
enum Backing {
    Disk(PathBuf),
    Memory {
        payloads: BTreeMap<PlacementId, Vec<u8>>,
        fail_writes: bool,
    },
}

struct State_ {
    index: BTreeMap<PlacementId, Record>,
    backing: Backing,
}

/// The placement store.
pub struct Store {
    limits: Limits,
    state: Mutex<State_>,
}

impl Store {
    /// A volatile store for tests and simulations.
    pub fn volatile(limits: Limits) -> Self {
        tracing::trace!("Creating a volatile placement store");
        Self {
            limits,
            state: Mutex::new(State_ {
                index: BTreeMap::new(),
                backing: Backing::Memory {
                    payloads: BTreeMap::new(),
                    fail_writes: false,
                },
            }),
        }
    }

    /// Open (creating when absent) a native store in `dir`, loading every record.
    /// # Errors
    /// `Storage` when the directory cannot be created or a record file is unreadable, oversized, malformed or inconsistent
    /// with its payload; nothing is repaired or replaced.
    pub fn open(dir: &Path, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Opening the native placement store");
        native::create_dir(dir)?;
        let index = native::load(dir, limits)?;
        Ok(Self {
            limits,
            state: Mutex::new(State_ {
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

    /// Insert a new placement durably; never replaces an existing one.
    /// # Errors
    /// `Conflict` when the placement ID exists, `Limit` when a finite bound would be exceeded, `Storage` when the write fails
    /// (nothing is recorded).
    pub fn insert(&self, placement: &Placement) -> Result<(), Error> {
        tracing::trace!("Inserting a placement");
        let mut state = self.lock();
        let id = placement.record.id;
        if state.index.contains_key(&id) {
            return Err(Error::Conflict);
        }
        if state.index.len() >= self.limits.placements
            || self
                .active_bytes(&state)
                .saturating_add(placement.payload.len() as u64)
                > self.limits.payload_bytes
        {
            return Err(Error::Limit);
        }
        write_payload(&mut state.backing, id, &placement.payload)?;
        if let Err(error) = write_record(&mut state.backing, &placement.record) {
            discard_payload(&mut state.backing, id);
            return Err(error);
        }
        state.index.insert(id, placement.record.clone());
        Ok(())
    }

    /// The record of a placement, without its payload.
    pub fn record(&self, id: PlacementId) -> Option<Record> {
        tracing::trace!("Reading a placement record");
        self.lock().index.get(&id).cloned()
    }

    /// A placement with its payload.
    /// # Errors
    /// `Storage` when the payload is missing or the wrong size (never returned partly).
    pub fn get(&self, id: PlacementId) -> Result<Option<Placement>, Error> {
        tracing::trace!("Reading a placement");
        let state = self.lock();
        let Some(record) = state.index.get(&id) else {
            return Ok(None);
        };
        let payload = read_payload(&state.backing, id).ok_or(Error::Storage)?;
        if payload.len() as u64 != record.metadata.payload_size {
            return Err(Error::Storage);
        }
        Ok(Some(Placement {
            record: record.clone(),
            payload,
        }))
    }

    /// Apply `change` to a record under the store lock and persist the result atomically. The closure may refuse; a refusal or a
    /// failed write leaves the previous record in place. Placement ID, message metadata and signature never change.
    /// # Errors
    /// `NotAvailable` for an unknown placement, the closure's own error, or `Storage` when the write fails.
    pub fn update(
        &self,
        id: PlacementId,
        change: &mut dyn FnMut(&mut Record) -> Result<(), Error>,
    ) -> Result<Record, Error> {
        tracing::trace!("Updating a placement record");
        let mut state = self.lock();
        let before = state.index.get(&id).cloned().ok_or(Error::NotAvailable)?;
        let mut after = before.clone();
        change(&mut after)?;
        after.id = before.id;
        after.metadata = before.metadata.clone();
        after.sender_public_key = before.sender_public_key;
        after.sender_signature = before.sender_signature;
        write_record(&mut state.backing, &after)?;
        state.index.insert(id, after.clone());
        Ok(after)
    }

    /// Pending placements for `recipient` ordered by `(accepted_at, placement_id)`, strictly after `after`, at most `limit`.
    pub fn pending_for(
        &self,
        recipient: IdentityId,
        after: Option<(u64, PlacementId)>,
        limit: usize,
    ) -> Vec<Record> {
        tracing::trace!("Listing pending placements for a recipient");
        let mut found: Vec<Record> = self
            .lock()
            .index
            .values()
            .filter(|r| r.state == State::Pending && r.metadata.recipient_id == recipient)
            .filter(|r| after.is_none_or(|key| (r.accepted_at, r.id) > key))
            .cloned()
            .collect();
        found.sort_by_key(|r| (r.accepted_at, r.id));
        found.truncate(limit);
        found
    }

    /// Pending placements by sender and by recipient, for placement quotas.
    pub fn pending_counts(&self, sender: IdentityId, recipient: IdentityId) -> (usize, usize) {
        tracing::trace!("Counting pending placements for quotas");
        let state = self.lock();
        let pending = |r: &&Record| r.state == State::Pending;
        (
            state
                .index
                .values()
                .filter(pending)
                .filter(|r| r.metadata.sender_id == sender)
                .count(),
            state
                .index
                .values()
                .filter(pending)
                .filter(|r| r.metadata.recipient_id == recipient)
                .count(),
        )
    }

    /// Every record, in identifier order.
    pub fn records(&self) -> Vec<Record> {
        tracing::trace!("Listing every placement record");
        self.lock().index.values().cloned().collect()
    }

    /// Release the payload bytes of a placement that no longer owes them; the record stays.
    /// # Errors
    /// `NotAvailable` for an unknown placement; `Storage` when the removal fails.
    pub fn release_payload(&self, id: PlacementId) -> Result<(), Error> {
        tracing::trace!("Releasing a placement payload");
        let mut state = self.lock();
        if !state.index.contains_key(&id) {
            return Err(Error::NotAvailable);
        }
        discard_payload(&mut state.backing, id);
        Ok(())
    }

    // Payload bytes of placements that still owe them.
    fn active_bytes(&self, state: &State_) -> u64 {
        state
            .index
            .values()
            .filter(|r| r.state.is_active())
            .map(|r| r.metadata.payload_size)
            .fold(0u64, u64::saturating_add)
    }

    // A poisoned lock still holds a consistent index because every update replaces one whole record.
    fn lock(&self) -> std::sync::MutexGuard<'_, State_> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// Write a payload: memory or an owner-only file replaced atomically.
fn write_payload(backing: &mut Backing, id: PlacementId, payload: &[u8]) -> Result<(), Error> {
    match backing {
        Backing::Memory {
            payloads,
            fail_writes,
        } => {
            if *fail_writes {
                return Err(Error::Storage);
            }
            payloads.insert(id, payload.to_vec());
            Ok(())
        }
        Backing::Disk(dir) => native::write_atomic(&native::payload_path(dir, id), payload),
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
            native::write_atomic(&native::record_path(dir, record.id), &bytes)
        }
    }
}

fn read_payload(backing: &Backing, id: PlacementId) -> Option<Vec<u8>> {
    match backing {
        Backing::Memory { payloads, .. } => payloads.get(&id).cloned(),
        Backing::Disk(dir) => std::fs::read(native::payload_path(dir, id)).ok(),
    }
}

fn discard_payload(backing: &mut Backing, id: PlacementId) {
    match backing {
        Backing::Memory { payloads, .. } => {
            payloads.remove(&id);
        }
        Backing::Disk(dir) => {
            let _ = std::fs::remove_file(native::payload_path(dir, id));
        }
    }
}

// Lowercase hex of bytes.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// Strict lowercase hex of exactly N bytes.
fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

// JSON form of a record.
fn record_json(r: &Record) -> Value {
    let m = &r.metadata;
    json!({
        "version": 1, "id": hex(&r.id.0), "messageId": hex(&m.message_id.0),
        "senderId": text::format(m.sender_id), "recipientId": text::format(m.recipient_id),
        "senderCreatedAt": m.sender_created_at, "encoding": m.encoding.name(),
        "payloadSize": m.payload_size, "payloadHash": hex(&m.payload_hash.0),
        "senderPublicKey": hex(&r.sender_public_key), "senderSignature": hex(&r.sender_signature.0),
        "acceptedAt": r.accepted_at, "expiresAt": r.expires_at,
        "state": state_name(r.state), "payments": r.payments,
        "receipt": r.receipt.as_ref().map(|x| json!({
            "recoveryId": hex(&x.recovery_id.0), "recoveredAt": x.recovered_at,
            "recipientPublicKey": hex(&x.recipient_public_key),
            "recipientSignature": hex(&x.recipient_signature.0),
            "providerSignature": hex(&x.provider_signature.0),
        })),
        "removal": r.removal.as_ref().map(|x| json!({
            "removalId": hex(&x.removal_id.0), "releaseCredits": x.release_credits, "price": x.price,
        })),
    })
}

fn state_name(state: State) -> &'static str {
    match state {
        State::Receiving => "receiving",
        State::Pending => "pending",
        State::Recovered => "recovered",
        State::Removed => "removed",
        State::Expired => "expired",
    }
}

// Parse a stored record strictly; any deviation is corruption.
fn parse_record(value: &Value) -> Option<Record> {
    let o = value.as_object()?;
    if o.len() != 17 || o.get("version")?.as_u64() != Some(1) {
        return None;
    }
    let text_of = |name: &str| o.get(name)?.as_str();
    let number = |name: &str| o.get(name)?.as_u64();
    let state = match text_of("state")? {
        "receiving" => State::Receiving,
        "pending" => State::Pending,
        "recovered" => State::Recovered,
        "removed" => State::Removed,
        "expired" => State::Expired,
        _ => return None,
    };
    let receipt = match o.get("receipt")? {
        Value::Null => None,
        r => {
            let r = r.as_object().filter(|r| r.len() == 5)?;
            Some(Receipt {
                recovery_id: super::ids::RecoveryId(unhex(r.get("recoveryId")?.as_str()?)?),
                recovered_at: r.get("recoveredAt")?.as_u64()?,
                recipient_public_key: unhex(r.get("recipientPublicKey")?.as_str()?)?,
                recipient_signature: Signature(unhex(r.get("recipientSignature")?.as_str()?)?),
                provider_signature: Signature(unhex(r.get("providerSignature")?.as_str()?)?),
            })
        }
    };
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
    Some(Record {
        id: PlacementId(unhex(text_of("id")?)?),
        metadata: Metadata {
            message_id: MessageId(unhex(text_of("messageId")?)?),
            sender_id: text::parse(text_of("senderId")?).ok()?,
            recipient_id: text::parse(text_of("recipientId")?).ok()?,
            sender_created_at: number("senderCreatedAt")?,
            encoding: Encoding::parse(text_of("encoding")?)?,
            payload_size: number("payloadSize")?,
            payload_hash: PayloadHash(unhex(text_of("payloadHash")?)?),
        },
        sender_public_key: unhex(text_of("senderPublicKey")?)?,
        sender_signature: Signature(unhex(text_of("senderSignature")?)?),
        accepted_at: number("acceptedAt")?,
        expires_at: number("expiresAt")?,
        state,
        payments: number("payments")?,
        receipt,
        removal,
    })
}

// Native file operations: owner-only directory, atomic replacement and a strict loader.
mod native {
    use super::*;
    use std::{
        io::Write,
        os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    };

    /// Largest accepted record file.
    const MAXIMUM_RECORD_BYTES: u64 = 16 * 1024;

    pub(super) fn record_path(dir: &Path, id: PlacementId) -> PathBuf {
        dir.join(format!("{}.json", hex(&id.0)))
    }
    pub(super) fn payload_path(dir: &Path, id: PlacementId) -> PathBuf {
        dir.join(format!("{}.payload", hex(&id.0)))
    }

    pub(super) fn create_dir(dir: &Path) -> Result<(), Error> {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(|_| Error::Storage)
    }

    // Write a private temporary file, flush it and rename it over the target.
    pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
        let temporary = path.with_extension("tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| Error::Storage)?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| Error::Storage)?;
        std::fs::rename(&temporary, path).map_err(|_| Error::Storage)
    }

    // Load every record, verifying that each active record's payload exists with the declared size.
    pub(super) fn load(dir: &Path, limits: Limits) -> Result<BTreeMap<PlacementId, Record>, Error> {
        let mut index = BTreeMap::new();
        for entry in std::fs::read_dir(dir).map_err(|_| Error::Storage)? {
            let path = entry.map_err(|_| Error::Storage)?.path();
            match path.extension().and_then(|e| e.to_str()) {
                Some("json") => {}
                // A leftover temporary file is an interrupted write; payloads are checked against their records.
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
                let length = std::fs::metadata(payload_path(dir, record.id))
                    .map_err(|_| Error::Storage)?
                    .len();
                if length != record.metadata.payload_size {
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
mod tests;
