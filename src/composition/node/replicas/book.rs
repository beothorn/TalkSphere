//! Durable record of hosted items and the independent replicas that protect them.
//!
//! Callers: the wanted work source hands each contributing client an item that still needs copies,
//! and the completion handler confirms a replica only after the Storage result validated. The book
//! counts a copy only once confirmed, never an attempt, and never gives one item to the same
//! identity twice, so confirmed copies are at distinct provider identities. Each item's sealed bytes
//! are kept in the replicas directory so repairs never depend on the replicas themselves. Everything
//! is bounded and owner-only; the book file is replaced atomically.
use crate::{
    composition::node::control::protocol::{hex, unhex},
    modules::identity::{api::IdentityId, text},
};
use serde_json::{Value, json};
use std::{
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Most items the book holds.
pub const MAXIMUM_ITEMS: usize = 1024;
/// Most replicas recorded for one item.
pub const MAXIMUM_REPLICAS: usize = 64;
/// Largest accepted book file.
const MAXIMUM_FILE_BYTES: u64 = 4 * 1_048_576;
const BOOK_FILE: &str = "book.json";

/// One confirmed replica: a validated lease at a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replica {
    /// Identity of the holder.
    pub provider: IdentityId,
    /// Lease identifier at that holder.
    pub lease: [u8; 32],
    /// Unix seconds when the lease expires.
    pub expires_at: u64,
    /// Unix seconds when the copy was confirmed.
    pub confirmed_at: u64,
    /// Unix seconds of the last conclusive check, if any.
    pub last_checked: Option<u64>,
}

/// A hosted item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Random identifier.
    pub id: [u8; 16],
    /// Plaintext length in bytes.
    pub size: u64,
    /// Length of the sealed bytes every replica holds.
    pub sealed_size: u64,
    /// SHA-256 of the plaintext, to verify a recovered item.
    pub plaintext_hash: [u8; 32],
    /// Unix seconds when the item was added.
    pub created_at: u64,
    /// Confirmed replicas.
    pub replicas: Vec<Replica>,
}

/// Why a book operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookError {
    /// The book file or an item file is unreadable, oversized or malformed.
    Corrupt,
    /// A file could not be written.
    Write,
    /// The book already holds the most items.
    Full,
    /// The item or reservation does not exist, or the item already exists.
    Unknown,
    /// The replica would repeat a holder or exceed the bound.
    Conflict,
}

struct State {
    items: Vec<Item>,
    reserved: Vec<([u8; 16], IdentityId)>,
}

/// The replica book in one directory.
pub struct Book {
    dir: PathBuf,
    state: Mutex<State>,
}

impl Book {
    /// Open the book in `dir`, creating the owner-only directory when absent.
    /// # Errors
    /// `Corrupt` for an unreadable, oversized or malformed book file, `Write` when the directory cannot be made.
    pub fn open(dir: &Path) -> Result<Self, BookError> {
        tracing::trace!("Opening the replica book");
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(|_| BookError::Write)?;
        let file = dir.join(BOOK_FILE);
        let items = match std::fs::metadata(&file) {
            Ok(meta) if meta.len() > MAXIMUM_FILE_BYTES => return Err(BookError::Corrupt),
            Ok(_) => parse(&std::fs::read(&file).map_err(|_| BookError::Corrupt)?)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(_) => return Err(BookError::Corrupt),
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            state: Mutex::new(State {
                items,
                reserved: Vec::new(),
            }),
        })
    }

    /// Add an item with its sealed bytes; the bytes are written before the item is recorded.
    /// # Errors
    /// `Full`, `Unknown` for a repeated identifier, or `Write`.
    pub fn add(&self, item: Item, sealed: &[u8]) -> Result<(), BookError> {
        tracing::trace!("Adding an item to the replica book");
        let mut state = self.lock();
        if state.items.len() >= MAXIMUM_ITEMS {
            return Err(BookError::Full);
        }
        if state.items.iter().any(|i| i.id == item.id) {
            return Err(BookError::Unknown);
        }
        write_private(&self.sealed_path(&item.id), sealed)?;
        state.items.push(item);
        if let Err(error) = self.persist(&state.items) {
            let removed = state.items.pop().map(|i| i.id);
            if let Some(id) = removed {
                let _ = std::fs::remove_file(self.sealed_path(&id));
            }
            return Err(error);
        }
        Ok(())
    }

    /// The sealed bytes of an item.
    /// # Errors
    /// `Unknown` for an absent item, `Corrupt` when the file is unreadable or the wrong size.
    pub fn sealed(&self, id: [u8; 16]) -> Result<Vec<u8>, BookError> {
        tracing::trace!("Reading the sealed bytes of an item");
        let size = self.get(id).ok_or(BookError::Unknown)?.sealed_size;
        let bytes = std::fs::read(self.sealed_path(&id)).map_err(|_| BookError::Corrupt)?;
        (bytes.len() as u64 == size)
            .then_some(bytes)
            .ok_or(BookError::Corrupt)
    }

    /// One item by identifier.
    pub fn get(&self, id: [u8; 16]) -> Option<Item> {
        tracing::trace!("Reading one replica item");
        self.lock().items.iter().find(|i| i.id == id).cloned()
    }

    /// Every item, oldest first.
    pub fn list(&self) -> Vec<Item> {
        tracing::trace!("Listing replica items");
        self.lock().items.clone()
    }

    /// Reserve the item most in need of a copy for `provider`: the one with the fewest confirmed plus
    /// reserved copies below `target`, oldest first, that the provider neither holds nor has reserved and
    /// whose sealed bytes fit `maximum_bytes`. Returns the item identifier and its sealed bytes.
    pub fn take_for(
        &self,
        provider: IdentityId,
        target: usize,
        maximum_bytes: u64,
    ) -> Option<([u8; 16], Vec<u8>)> {
        tracing::trace!("Reserving an item for a contributing provider");
        let mut state = self.lock();
        let held = |item: &Item, reserved: &[([u8; 16], IdentityId)]| {
            item.replicas.len() + reserved.iter().filter(|(id, _)| *id == item.id).count()
        };
        let reserved = state.reserved.clone();
        let chosen = state
            .items
            .iter()
            .filter(|i| i.sealed_size <= maximum_bytes && held(i, &reserved) < target)
            .filter(|i| !i.replicas.iter().any(|r| r.provider == provider))
            .filter(|i| !reserved.contains(&(i.id, provider)))
            .min_by_key(|i| (held(i, &reserved), i.created_at, i.id))
            .map(|i| i.id)?;
        let bytes = std::fs::read(self.sealed_path(&chosen)).ok()?;
        state.reserved.push((chosen, provider));
        Some((chosen, bytes))
    }

    /// Give back a reservation without recording a replica.
    pub fn release(&self, id: [u8; 16], provider: IdentityId) {
        tracing::trace!("Releasing an item reservation");
        self.lock().reserved.retain(|r| *r != (id, provider));
    }

    /// Record a confirmed replica and end the provider's reservation.
    /// # Errors
    /// `Unknown` when the item is absent, `Conflict` when the provider already holds it or the bound is reached,
    /// `Write` when the book cannot be written (the replica is then not recorded).
    pub fn confirm(&self, id: [u8; 16], replica: Replica) -> Result<(), BookError> {
        tracing::trace!("Confirming a replica");
        let mut state = self.lock();
        let index = state
            .items
            .iter()
            .position(|i| i.id == id)
            .ok_or(BookError::Unknown)?;
        let item = &state.items[index];
        if item.replicas.len() >= MAXIMUM_REPLICAS
            || item.replicas.iter().any(|r| r.provider == replica.provider)
        {
            return Err(BookError::Conflict);
        }
        let provider = replica.provider;
        state.items[index].replicas.push(replica);
        if let Err(error) = self.persist(&state.items) {
            state.items[index].replicas.pop();
            return Err(error);
        }
        state.reserved.retain(|r| *r != (id, provider));
        Ok(())
    }

    /// Forget the replica held by `provider`, for example because it expired or failed a check; the item
    /// then needs another copy.
    /// # Errors
    /// `Unknown` when the item or replica is absent, `Write` when the book cannot be written (the replica
    /// is then kept).
    pub fn drop_replica(&self, id: [u8; 16], provider: IdentityId) -> Result<(), BookError> {
        tracing::trace!("Dropping a replica from the book");
        let mut state = self.lock();
        let (index, at) = locate(&state.items, id, provider)?;
        let removed = state.items[index].replicas.remove(at);
        if let Err(error) = self.persist(&state.items) {
            state.items[index].replicas.insert(at, removed);
            return Err(error);
        }
        Ok(())
    }

    /// Change one replica's recorded expiry or last check.
    /// # Errors
    /// `Unknown` when the item or replica is absent, `Write` when the book cannot be written (the change is
    /// then undone).
    pub fn update_replica(
        &self,
        id: [u8; 16],
        provider: IdentityId,
        change: impl FnOnce(&mut Replica),
    ) -> Result<(), BookError> {
        tracing::trace!("Updating a replica in the book");
        let mut state = self.lock();
        let (index, at) = locate(&state.items, id, provider)?;
        let before = state.items[index].replicas[at].clone();
        change(&mut state.items[index].replicas[at]);
        // The holder and lease identify the copy and never change.
        state.items[index].replicas[at].provider = before.provider;
        state.items[index].replicas[at].lease = before.lease;
        if let Err(error) = self.persist(&state.items) {
            state.items[index].replicas[at] = before;
            return Err(error);
        }
        Ok(())
    }

    // Path of an item's sealed bytes.
    fn sealed_path(&self, id: &[u8; 16]) -> PathBuf {
        self.dir.join(format!("{}.sealed", hex(id)))
    }

    // Replace the book file atomically.
    fn persist(&self, items: &[Item]) -> Result<(), BookError> {
        let document =
            json!({"version": 1, "items": items.iter().map(item_json).collect::<Vec<_>>()});
        let bytes = serde_json::to_vec(&document).map_err(|_| BookError::Write)?;
        let file = self.dir.join(BOOK_FILE);
        let temporary = file.with_extension("tmp");
        write_private(&temporary, &bytes)?;
        std::fs::rename(&temporary, &file).map_err(|_| BookError::Write)
    }

    // A poisoned lock still holds consistent state because every update is one push, pop or retain.
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// The positions of one item and of one holder's replica within it.
fn locate(items: &[Item], id: [u8; 16], provider: IdentityId) -> Result<(usize, usize), BookError> {
    let index = items
        .iter()
        .position(|i| i.id == id)
        .ok_or(BookError::Unknown)?;
    let at = items[index]
        .replicas
        .iter()
        .position(|r| r.provider == provider)
        .ok_or(BookError::Unknown)?;
    Ok((index, at))
}

// Write a new or replaced file with owner-only permissions and flush it.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), BookError> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| BookError::Write)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| BookError::Write)
}

// JSON form of one item.
fn item_json(item: &Item) -> Value {
    json!({
        "id": hex(&item.id), "size": item.size, "sealedSize": item.sealed_size,
        "plaintextHash": hex(&item.plaintext_hash), "createdAt": item.created_at,
        "replicas": item.replicas.iter().map(|r| json!({
            "provider": text::format(r.provider), "lease": hex(&r.lease),
            "expiresAt": r.expires_at, "confirmedAt": r.confirmed_at, "lastChecked": r.last_checked,
        })).collect::<Vec<_>>(),
    })
}

// Parse the stored book strictly.
fn parse(bytes: &[u8]) -> Result<Vec<Item>, BookError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| BookError::Corrupt)?;
    let root = value.as_object().ok_or(BookError::Corrupt)?;
    if root.len() != 2 || root.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(BookError::Corrupt);
    }
    let list = root
        .get("items")
        .and_then(Value::as_array)
        .ok_or(BookError::Corrupt)?;
    if list.len() > MAXIMUM_ITEMS {
        return Err(BookError::Corrupt);
    }
    let items: Vec<Item> = list.iter().map(parse_item).collect::<Result<_, _>>()?;
    let mut ids: Vec<&[u8; 16]> = items.iter().map(|i| &i.id).collect();
    ids.sort();
    ids.dedup();
    if ids.len() != items.len() {
        return Err(BookError::Corrupt);
    }
    Ok(items)
}

// One stored item with exact members.
fn parse_item(value: &Value) -> Result<Item, BookError> {
    let o = value.as_object().ok_or(BookError::Corrupt)?;
    if o.len() != 6 {
        return Err(BookError::Corrupt);
    }
    let number = |name: &str| {
        o.get(name)
            .and_then(Value::as_u64)
            .ok_or(BookError::Corrupt)
    };
    let fixed = |name: &str, n: usize| {
        o.get(name)
            .and_then(Value::as_str)
            .and_then(unhex)
            .filter(|b| b.len() == n)
            .ok_or(BookError::Corrupt)
    };
    let replicas = o
        .get("replicas")
        .and_then(Value::as_array)
        .ok_or(BookError::Corrupt)?;
    if replicas.len() > MAXIMUM_REPLICAS {
        return Err(BookError::Corrupt);
    }
    let replicas: Vec<Replica> = replicas
        .iter()
        .map(parse_replica)
        .collect::<Result<_, _>>()?;
    for (index, replica) in replicas.iter().enumerate() {
        if replicas[..index]
            .iter()
            .any(|r| r.provider == replica.provider)
        {
            return Err(BookError::Corrupt);
        }
    }
    Ok(Item {
        id: fixed("id", 16)?
            .try_into()
            .map_err(|_| BookError::Corrupt)?,
        size: number("size")?,
        sealed_size: number("sealedSize")?,
        plaintext_hash: fixed("plaintextHash", 32)?
            .try_into()
            .map_err(|_| BookError::Corrupt)?,
        created_at: number("createdAt")?,
        replicas,
    })
}

// One stored replica with exact members.
fn parse_replica(value: &Value) -> Result<Replica, BookError> {
    let o = value.as_object().ok_or(BookError::Corrupt)?;
    if o.len() != 5 {
        return Err(BookError::Corrupt);
    }
    let number = |name: &str| {
        o.get(name)
            .and_then(Value::as_u64)
            .ok_or(BookError::Corrupt)
    };
    let last_checked = match o.get("lastChecked") {
        Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().ok_or(BookError::Corrupt)?),
        None => return Err(BookError::Corrupt),
    };
    Ok(Replica {
        provider: o
            .get("provider")
            .and_then(Value::as_str)
            .and_then(|t| text::parse(t).ok())
            .ok_or(BookError::Corrupt)?,
        lease: o
            .get("lease")
            .and_then(Value::as_str)
            .and_then(unhex)
            .and_then(|b| b.try_into().ok())
            .ok_or(BookError::Corrupt)?,
        expires_at: number("expiresAt")?,
        confirmed_at: number("confirmedAt")?,
        last_checked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A fresh private directory per test.
    fn dir() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        std::env::temp_dir().join(format!(
            "ts-book-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    // An item with `n` as identifier and a small sealed size.
    fn item(n: u8, created_at: u64) -> Item {
        Item {
            id: [n; 16],
            size: 3,
            sealed_size: 5,
            plaintext_hash: [n; 32],
            created_at,
            replicas: vec![],
        }
    }

    // A confirmed replica at provider `p`.
    fn replica(p: u8) -> Replica {
        Replica {
            provider: IdentityId([p; 32]),
            lease: [p; 32],
            expires_at: 100,
            confirmed_at: 10,
            last_checked: None,
        }
    }

    // Items and confirmed replicas survive a reopen, with their sealed bytes and in order.
    #[test]
    fn the_book_persists_across_reopen() {
        let path = dir();
        let book = Book::open(&path).unwrap();
        book.add(item(1, 5), b"AAAAA").unwrap();
        book.add(item(2, 6), b"BBBBB").unwrap();
        book.confirm([1; 16], replica(9)).unwrap();
        let reopened = Book::open(&path).unwrap();
        assert_eq!(reopened.list().len(), 2);
        assert_eq!(reopened.get([1; 16]).unwrap().replicas, vec![replica(9)]);
        assert_eq!(reopened.sealed([2; 16]).unwrap(), b"BBBBB");
        assert_eq!(reopened.add(item(1, 7), b"CCCCC"), Err(BookError::Unknown));
    }

    // The neediest item goes first, a provider never gets an item twice, and the target bounds copies.
    #[test]
    fn items_go_to_distinct_providers_up_to_the_target() {
        let book = Book::open(&dir()).unwrap();
        book.add(item(1, 5), b"AAAAA").unwrap();
        book.add(item(2, 6), b"BBBBB").unwrap();
        let p = |n: u8| IdentityId([n; 32]);
        let (first, bytes) = book.take_for(p(1), 2, 100).unwrap();
        assert_eq!((first, bytes), ([1; 16], b"AAAAA".to_vec()));
        // Item 2 has fewer copies in flight than item 1 now, and provider 1 may not hold item 1 twice.
        assert_eq!(book.take_for(p(1), 2, 100).unwrap().0, [2; 16]);
        assert!(book.take_for(p(1), 2, 100).is_none());
        // A second provider gets the item with the fewest copies (a tie is broken by age).
        assert_eq!(book.take_for(p(2), 2, 100).unwrap().0, [1; 16]);
        book.confirm([1; 16], replica(1)).unwrap();
        book.confirm([1; 16], replica(2)).unwrap();
        // Item 1 is at its target of two; a third provider only gets item 2.
        assert_eq!(book.take_for(p(3), 2, 100).unwrap().0, [2; 16]);
        assert!(
            book.take_for(p(4), 2, 100).is_none(),
            "both items now have their copies confirmed or in flight"
        );
    }

    // Releasing a reservation makes the item available again; oversize items are never given out.
    #[test]
    fn released_reservations_free_the_item_and_size_is_bounded() {
        let book = Book::open(&dir()).unwrap();
        book.add(item(1, 5), b"AAAAA").unwrap();
        let p = IdentityId([1; 32]);
        assert!(
            book.take_for(p, 1, 4).is_none(),
            "the sealed size exceeds the bound"
        );
        assert!(book.take_for(p, 1, 5).is_some());
        assert!(
            book.take_for(IdentityId([2; 32]), 1, 5).is_none(),
            "the target counts reservations"
        );
        book.release([1; 16], p);
        assert!(book.take_for(IdentityId([2; 32]), 1, 5).is_some());
    }

    // A holder cannot be recorded twice, and an unknown item is refused.
    #[test]
    fn confirmation_refuses_repeats_and_unknown_items() {
        let book = Book::open(&dir()).unwrap();
        book.add(item(1, 5), b"AAAAA").unwrap();
        book.confirm([1; 16], replica(1)).unwrap();
        assert_eq!(book.confirm([1; 16], replica(1)), Err(BookError::Conflict));
        assert_eq!(book.confirm([9; 16], replica(2)), Err(BookError::Unknown));
        for p in 2..=MAXIMUM_REPLICAS as u8 {
            book.confirm([1; 16], replica(p)).unwrap();
        }
        assert_eq!(
            book.confirm([1; 16], replica(200)),
            Err(BookError::Conflict)
        );
    }

    // The item bound holds, and a missing or truncated sealed file is reported, never returned.
    #[test]
    fn bounds_and_damaged_files_are_reported() {
        let path = dir();
        let book = Book::open(&path).unwrap();
        // Fill the table directly: writing a thousand synced files would only slow the test down.
        for n in 0..MAXIMUM_ITEMS - 1 {
            let mut it = item(0, n as u64);
            it.id[..8].copy_from_slice(&(n as u64).to_be_bytes());
            book.lock().items.push(it);
        }
        book.add(item(255, 0), b"AAAAA").unwrap();
        assert_eq!(book.add(item(254, 0), b"AAAAA"), Err(BookError::Full));
        let id = [255; 16];
        std::fs::write(path.join(format!("{}.sealed", hex(&id))), b"AA").unwrap();
        assert_eq!(book.sealed(id), Err(BookError::Corrupt));
        std::fs::remove_file(path.join(format!("{}.sealed", hex(&id)))).unwrap();
        assert_eq!(book.sealed(id), Err(BookError::Corrupt));
        assert_eq!(book.sealed([7; 16]), Err(BookError::Unknown));
    }

    // Corrupt, duplicated and oversized book files are refused and left alone.
    #[test]
    fn corrupt_book_files_are_refused() {
        let path = dir();
        let book = Book::open(&path).unwrap();
        book.add(item(1, 5), b"AAAAA").unwrap();
        let file = path.join(BOOK_FILE);
        let good = std::fs::read(&file).unwrap();
        for bad in [
            b"nonsense".to_vec(),
            br#"{"version":2,"items":[]}"#.to_vec(),
            br#"{"version":1,"items":[],"extra":1}"#.to_vec(),
            String::from_utf8(good.clone())
                .unwrap()
                .replace("\"size\":3", "\"size\":-3")
                .into_bytes(),
        ] {
            std::fs::write(&file, &bad).unwrap();
            assert_eq!(Book::open(&path).err(), Some(BookError::Corrupt));
            assert_eq!(std::fs::read(&file).unwrap(), bad);
        }
        let duplicate = serde_json::to_vec(&json!({"version":1,"items":[
            item_json(&item(1, 5)), item_json(&item(1, 6))]}))
        .unwrap();
        std::fs::write(&file, duplicate).unwrap();
        assert_eq!(Book::open(&path).err(), Some(BookError::Corrupt));
        std::fs::write(&file, vec![b' '; MAXIMUM_FILE_BYTES as usize + 1]).unwrap();
        assert_eq!(Book::open(&path).err(), Some(BookError::Corrupt));
    }

    // A replica can be dropped (so the item needs another copy) or updated, but never re-pointed at another
    // holder or lease; unknown items and holders are refused; changes persist.
    #[test]
    fn replicas_can_be_dropped_and_updated() {
        let path = dir();
        let book = Book::open(&path).unwrap();
        book.add(item(1, 5), b"AAAAA").unwrap();
        book.confirm([1; 16], replica(1)).unwrap();
        book.confirm([1; 16], replica(2)).unwrap();
        book.update_replica([1; 16], IdentityId([1; 32]), |r| {
            r.expires_at = 500;
            r.last_checked = Some(40);
            r.provider = IdentityId([9; 32]);
            r.lease = [9; 32];
        })
        .unwrap();
        let reopened = Book::open(&path).unwrap();
        let held = &reopened.get([1; 16]).unwrap().replicas[0];
        assert_eq!((held.expires_at, held.last_checked), (500, Some(40)));
        assert_eq!((held.provider, held.lease), (IdentityId([1; 32]), [1; 32]));
        book.drop_replica([1; 16], IdentityId([1; 32])).unwrap();
        assert_eq!(
            Book::open(&path)
                .unwrap()
                .get([1; 16])
                .unwrap()
                .replicas
                .len(),
            1
        );
        assert_eq!(
            book.drop_replica([1; 16], IdentityId([1; 32])),
            Err(BookError::Unknown)
        );
        assert_eq!(
            book.drop_replica([8; 16], IdentityId([2; 32])),
            Err(BookError::Unknown)
        );
        // The dropped holder may offer a fresh copy again.
        assert!(book.take_for(IdentityId([1; 32]), 2, 100).is_some());
    }

    // Files are owner-only.
    #[test]
    fn files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let path = dir();
        let book = Book::open(&path).unwrap();
        book.add(item(1, 5), b"AAAAA").unwrap();
        for name in [BOOK_FILE.to_string(), format!("{}.sealed", hex(&[1; 16]))] {
            assert_eq!(
                std::fs::metadata(path.join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}
