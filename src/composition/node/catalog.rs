//! Durable requester lease catalog: the application's own record of leases bought from providers.
//!
//! Callers: the node's Storage requester port and local control commands. Entries are
//! written only after a full result validation by the Storage requester contracts, are
//! scoped to their requester and provider, and are replaced atomically on disk. A provider
//! never receives this catalog and it holds no encryption keys.
use crate::modules::{
    identity::{api::IdentityId, text},
    services::{Error, contract::OperationKey, handler::Blob},
    storage::{
        contract::{Lease, State},
        requester::{catalog::Catalog, client::Leases},
    },
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Largest catalog file accepted when opening.
const MAXIMUM_FILE_BYTES: u64 = 16 * 1_048_576;
/// Largest retained accepted-terms blob.
const MAXIMUM_TERMS_BYTES: usize = 65_536;

/// One cataloged lease and where its provider was last reached.
#[derive(Clone)]
pub struct Entry {
    /// Validated lease snapshot.
    pub lease: Lease,
    /// Provider contact text (`ID?address=..&port=..`) used for later operations.
    pub contact: String,
}

/// Failure of catalog persistence or parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogError {
    /// The file is unreadable, oversized or malformed.
    Corrupt,
    /// The file could not be written.
    Write,
}

/// File-backed lease catalog; one writer per node root.
pub struct FileCatalog {
    path: PathBuf,
    local: IdentityId,
    entries: Mutex<BTreeMap<(IdentityId, [u8; 32]), Entry>>,
}

impl FileCatalog {
    /// Open an existing catalog or start an empty one when the file is absent.
    /// # Errors
    /// A present file that is oversized, malformed or for another requester is `Corrupt`.
    pub fn open(path: &Path, local: IdentityId) -> Result<Self, CatalogError> {
        tracing::trace!("Opening the requester lease catalog");
        let entries = match std::fs::metadata(path) {
            Ok(meta) if meta.len() > MAXIMUM_FILE_BYTES => return Err(CatalogError::Corrupt),
            Ok(_) => parse(
                &std::fs::read(path).map_err(|_| CatalogError::Corrupt)?,
                local,
            )?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(_) => return Err(CatalogError::Corrupt),
        };
        Ok(Self {
            path: path.to_path_buf(),
            local,
            entries: Mutex::new(entries),
        })
    }

    /// Write an empty catalog file for a newly initialized node.
    /// # Errors
    /// An existing file or a write failure.
    pub fn create(path: &Path, local: IdentityId) -> Result<Self, CatalogError> {
        tracing::trace!("Creating an empty requester lease catalog");
        if path.exists() {
            return Err(CatalogError::Write);
        }
        let catalog = Self {
            path: path.to_path_buf(),
            local,
            entries: Mutex::new(BTreeMap::new()),
        };
        catalog.persist(&BTreeMap::new())?;
        Ok(catalog)
    }

    /// Retain or replace one validated lease with the provider contact used to reach it.
    /// A conflicting entry for another requester is refused; repeating the same entry is idempotent.
    /// # Errors
    /// `Write` when the atomic replacement fails; memory is unchanged in that case.
    pub fn put(&self, lease: &Lease, contact: &str) -> Result<(), CatalogError> {
        tracing::trace!("Retaining a validated lease in the catalog");
        if lease.requester != self.local {
            return Err(CatalogError::Corrupt);
        }
        let mut entries = self.entries.lock().map_err(|_| CatalogError::Write)?;
        let mut next = entries.clone();
        next.insert(
            (lease.provider, lease.id),
            Entry {
                lease: lease.clone(),
                contact: contact.to_string(),
            },
        );
        self.persist(&next)?;
        *entries = next;
        Ok(())
    }

    /// Look up one entry by provider and lease identifier.
    pub fn get(&self, provider: IdentityId, id: [u8; 32]) -> Option<Entry> {
        tracing::trace!("Reading one catalog entry");
        self.entries.lock().ok()?.get(&(provider, id)).cloned()
    }

    /// Find entries by lease identifier alone; more than one provider may use the same value.
    pub fn find(&self, id: [u8; 32]) -> Vec<Entry> {
        tracing::trace!("Searching the catalog by lease identifier");
        self.entries.lock().map_or_else(
            |_| Vec::new(),
            |e| e.values().filter(|v| v.lease.id == id).cloned().collect(),
        )
    }

    /// Return every entry in stable provider/ID order.
    pub fn list(&self) -> Vec<Entry> {
        tracing::trace!("Listing the lease catalog");
        self.entries
            .lock()
            .map_or_else(|_| Vec::new(), |e| e.values().cloned().collect())
    }

    // Replace the file atomically: write a private temporary, flush it, then rename over the catalog.
    fn persist(
        &self,
        entries: &BTreeMap<(IdentityId, [u8; 32]), Entry>,
    ) -> Result<(), CatalogError> {
        tracing::trace!("Persisting the lease catalog atomically");
        use std::os::unix::fs::OpenOptionsExt;
        let document = json!({
            "version": 1,
            "requester": text::format(self.local),
            "leases": entries.values().map(entry_json).collect::<Vec<_>>(),
        });
        let bytes = serde_json::to_vec(&document).map_err(|_| CatalogError::Write)?;
        let temporary = self.path.with_extension("tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| CatalogError::Write)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| CatalogError::Write)?;
        std::fs::rename(&temporary, &self.path).map_err(|_| CatalogError::Write)
    }
}

impl Catalog for FileCatalog {
    // The requester-contract hook has no contact; keep any existing one for this lease.
    fn retain(&self, lease: &Lease) -> Result<(), Error> {
        tracing::trace!("Retaining a lease through the requester catalog port");
        let contact = self
            .get(lease.provider, lease.id)
            .map(|e| e.contact)
            .unwrap_or_default();
        self.put(lease, &contact).map_err(|_| Error::Unavailable)
    }
}

impl Leases for FileCatalog {
    // Only the owning requester's exact provider-scoped record is visible; others look absent.
    fn lease(
        &self,
        key: OperationKey,
        provider: IdentityId,
        requester: IdentityId,
        id: [u8; 32],
    ) -> Result<Lease, Error> {
        tracing::trace!("Checking original lease expectations in the catalog");
        if key.requester != requester || requester != self.local {
            return Err(Error::Denied);
        }
        self.get(provider, id)
            .map(|e| e.lease)
            .filter(|l| l.requester == requester)
            .ok_or(Error::Denied)
    }
}

// Convert one entry to its stored JSON form with exact integers and hexadecimal byte fields.
fn entry_json(entry: &Entry) -> Value {
    let l = &entry.lease;
    json!({
        "id": hex(&l.id), "provider": text::format(l.provider), "contact": entry.contact,
        "acceptedTerms": hex(l.accepted_terms.bytes()), "sizeBytes": l.size_bytes,
        "contentHash": hex(&l.content_hash), "acceptedAt": l.accepted_at,
        "expiresAt": l.expires_at, "storePrice": l.store_price, "state": state_name(l.state),
    })
}

// Name a lease state in the stored document.
fn state_name(state: State) -> &'static str {
    match state {
        State::Receiving => "receiving",
        State::Active => "active",
        State::Expired => "expired",
        State::Removed => "removed",
    }
}

// Parse a stored catalog, refusing wrong owners, duplicates and any malformed field.
fn parse(
    bytes: &[u8],
    local: IdentityId,
) -> Result<BTreeMap<(IdentityId, [u8; 32]), Entry>, CatalogError> {
    tracing::trace!("Parsing the stored lease catalog");
    let bad = CatalogError::Corrupt;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| bad)?;
    if value["version"].as_u64() != Some(1) {
        return Err(bad);
    }
    let owner = text::parse(value["requester"].as_str().ok_or(bad)?).map_err(|_| bad)?;
    if owner != local {
        return Err(bad);
    }
    let mut entries = BTreeMap::new();
    for item in value["leases"].as_array().ok_or(bad)? {
        let field = |name: &str| item[name].as_u64().ok_or(bad);
        let provider = text::parse(item["provider"].as_str().ok_or(bad)?).map_err(|_| bad)?;
        let id: [u8; 32] = unhex(item["id"].as_str().ok_or(bad)?)?
            .try_into()
            .map_err(|_| bad)?;
        let hash: [u8; 32] = unhex(item["contentHash"].as_str().ok_or(bad)?)?
            .try_into()
            .map_err(|_| bad)?;
        let terms = Blob::new(
            unhex(item["acceptedTerms"].as_str().ok_or(bad)?)?,
            MAXIMUM_TERMS_BYTES,
        )
        .map_err(|_| bad)?;
        let state = match item["state"].as_str().ok_or(bad)? {
            "receiving" => State::Receiving,
            "active" => State::Active,
            "expired" => State::Expired,
            "removed" => State::Removed,
            _ => return Err(bad),
        };
        let lease = Lease {
            id,
            requester: local,
            provider,
            accepted_terms: terms,
            size_bytes: field("sizeBytes")?,
            content_hash: hash,
            accepted_at: field("acceptedAt")?,
            expires_at: field("expiresAt")?,
            store_price: field("storePrice")?,
            state,
        };
        let contact = item["contact"].as_str().ok_or(bad)?.to_string();
        if entries
            .insert((provider, id), Entry { lease, contact })
            .is_some()
        {
            return Err(bad);
        }
    }
    Ok(entries)
}

// Lowercase hexadecimal text of bytes.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// Decode strict lowercase hexadecimal text.
fn unhex(text: &str) -> Result<Vec<u8>, CatalogError> {
    if !text.len().is_multiple_of(2)
        || !text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(CatalogError::Corrupt);
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|_| CatalogError::Corrupt))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lease(provider: u8, id: u8, requester: IdentityId) -> Lease {
        Lease {
            id: [id; 32],
            requester,
            provider: IdentityId([provider; 32]),
            accepted_terms: Blob::new(vec![1, 2, 3], 16).unwrap(),
            size_bytes: 3,
            content_hash: [9; 32],
            accepted_at: 100,
            expires_at: 160,
            store_price: 7,
            state: State::Active,
        }
    }
    fn key(requester: IdentityId) -> OperationKey {
        OperationKey {
            requester,
            connection: [0; 32],
            stream: 1,
        }
    }
    fn dir() -> PathBuf {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!("catalog-{}", u64::from_le_bytes(nonce)));
        std::fs::create_dir(&path).unwrap();
        path
    }
    // Entries survive reopen exactly, keep their contact and replace an earlier version of the same lease.
    #[test]
    fn entries_persist_across_reopen_and_replace() {
        let path = dir().join("catalog.json");
        let me = IdentityId([1; 32]);
        let catalog = FileCatalog::create(&path, me).unwrap();
        catalog.put(&lease(2, 5, me), "contact-a").unwrap();
        let mut renewed = lease(2, 5, me);
        renewed.expires_at = 200;
        catalog.put(&renewed, "contact-a").unwrap();
        let reopened = FileCatalog::open(&path, me).unwrap();
        let entry = reopened.get(IdentityId([2; 32]), [5; 32]).unwrap();
        assert_eq!(
            (entry.lease.expires_at, entry.contact.as_str()),
            (200, "contact-a")
        );
        assert_eq!(reopened.list().len(), 1);
        assert_eq!(reopened.find([5; 32]).len(), 1);
    }
    // The same identifier at two providers stays separate, and foreign requesters cannot see or write entries.
    #[test]
    fn leases_are_scoped_to_provider_and_requester() {
        let path = dir().join("catalog.json");
        let me = IdentityId([1; 32]);
        let other = IdentityId([9; 32]);
        let catalog = FileCatalog::create(&path, me).unwrap();
        catalog.put(&lease(2, 5, me), "a").unwrap();
        catalog.put(&lease(3, 5, me), "b").unwrap();
        assert_eq!(catalog.find([5; 32]).len(), 2);
        assert!(
            catalog
                .lease(key(me), IdentityId([2; 32]), me, [5; 32])
                .is_ok()
        );
        assert_eq!(
            catalog
                .lease(key(other), IdentityId([2; 32]), other, [5; 32])
                .err(),
            Some(Error::Denied)
        );
        assert_eq!(
            catalog
                .lease(key(me), IdentityId([4; 32]), me, [5; 32])
                .err(),
            Some(Error::Denied)
        );
        assert_eq!(
            catalog.put(&lease(2, 6, other), "x"),
            Err(CatalogError::Corrupt)
        );
    }
    // Corrupt, foreign, duplicate and oversized files are refused rather than partly loaded.
    #[test]
    fn corrupt_files_are_refused() {
        let root = dir();
        let me = IdentityId([1; 32]);
        let path = root.join("catalog.json");
        let good = FileCatalog::create(&path, me).unwrap();
        good.put(&lease(2, 5, me), "a").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        for bad in [
            "not json".to_string(),
            text.replace("\"version\":1", "\"version\":2"),
            text.replace("\"state\":\"active\"", "\"state\":\"odd\""),
            text.replace("\"sizeBytes\":3", "\"sizeBytes\":\"3\""),
            text.replace(&"09".repeat(32), "zz"),
        ] {
            std::fs::write(&path, bad).unwrap();
            assert_eq!(
                FileCatalog::open(&path, me).err(),
                Some(CatalogError::Corrupt)
            );
        }
        std::fs::write(&path, &text).unwrap();
        assert!(FileCatalog::open(&path, IdentityId([8; 32])).is_err());
        assert!(FileCatalog::create(&path, me).is_err());
    }
    // A failed write leaves memory and the previous file unchanged.
    #[test]
    fn write_failure_changes_nothing() {
        let root = dir();
        let me = IdentityId([1; 32]);
        let path = root.join("catalog.json");
        let catalog = FileCatalog::create(&path, me).unwrap();
        catalog.put(&lease(2, 5, me), "a").unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(catalog.put(&lease(2, 6, me), "a"), Err(CatalogError::Write));
        assert_eq!(catalog.list().len(), 1);
    }
    // The catalog file is private to its owner.
    #[test]
    fn file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let path = dir().join("catalog.json");
        FileCatalog::create(&path, IdentityId([1; 32])).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
