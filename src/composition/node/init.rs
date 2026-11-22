//! Explicit node initialization and read-only inspection.
//!
//! Callers: the `node-init` and `node-show` commands. Initialization validates the
//! configuration before any effect, never replaces existing state, and leaves a
//! partly created root in place for inspection when a later stage fails.
use super::{
    config::{MAXIMUM_DOCUMENT_BYTES, NodeConfig},
    layout::Layout,
    owners,
};
use crate::composition::policy::Params;
use crate::modules::{
    cryptography::random::SystemEntropy,
    identity::{
        api::{IdentityId, IdentityProvider},
        config::Limits,
        provider::Identity,
        storage::FileStore,
        text,
    },
};
use std::{
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
};

/// Create a fresh node at `root` and return its public identity.
/// `config_document` is the operator's `node.json` content; `None` uses the defaults.
/// # Errors
/// An invalid configuration fails before any effect. An existing root with content is refused
/// untouched. Later failures leave the partly created root for `describe` and manual removal.
pub fn initialize(root: &Path, config_document: Option<&[u8]>) -> Result<IdentityId, String> {
    tracing::trace!("Initializing a new node root");
    let config = match config_document {
        Some(bytes) => NodeConfig::parse(bytes).map_err(|e| e.to_string())?,
        None => NodeConfig::default(),
    };
    let layout = Layout::new(root);
    prepare_root(root)?;
    let local = create_identity(&layout.identity())?;
    write_private(
        &layout.config(),
        &serde_json::to_vec_pretty(&config.to_json())
            .map_err(|_| "configuration could not be encoded")?,
    )?;
    write_private(&layout.public_id(), text::format(local).as_bytes())?;
    drop(owners::open(&layout, config, local, true)?);
    tracing::info!("Node initialized");
    Ok(local)
}

/// Read a bounded configuration document from a file for `initialize`.
/// # Errors
/// An unreadable or oversized file.
pub fn read_document(path: &Path) -> Result<Vec<u8>, String> {
    tracing::trace!("Reading an operator configuration file");
    let meta =
        std::fs::metadata(path).map_err(|_| "configuration file is unreadable".to_string())?;
    if meta.len() > MAXIMUM_DOCUMENT_BYTES as u64 {
        return Err("configuration file is too large".into());
    }
    std::fs::read(path).map_err(|_| "configuration file is unreadable".into())
}

/// Summarize an existing node root without starting it or taking any owner lock.
/// # Errors
/// The root does not exist or is not a directory.
pub fn describe(root: &Path) -> Result<String, String> {
    tracing::trace!("Inspecting a node root");
    if !root.is_dir() {
        return Err("node root is unavailable".into());
    }
    let layout = Layout::new(root);
    let mut lines = vec![format!("node: {}", root.display())];
    let identity = std::fs::read_to_string(layout.public_id()).ok();
    lines.push(format!(
        "identity: {}",
        identity.as_deref().unwrap_or("unknown")
    ));
    let config = std::fs::read(layout.config())
        .ok()
        .and_then(|bytes| NodeConfig::parse(&bytes).ok());
    match &config {
        Some(c) => {
            lines.push(format!(
                "role: {}",
                if c.offering.is_some() {
                    "provider and requester"
                } else {
                    "requester only"
                }
            ));
            lines.push(format!(
                "capacity: {} bytes, {} leases, {} pending operations",
                c.capacity.bytes, c.capacity.leases, c.capacity.operations
            ));
        }
        None => lines.push("configuration: missing or invalid".into()),
    }
    lines.push(match Params::load(&layout.policy()) {
        Ok(_) if layout.policy().exists() => "policy: policy.json valid".to_string(),
        Ok(_) => "policy: defaults (no policy.json)".to_string(),
        Err(_) => "policy: policy.json invalid".to_string(),
    });
    let missing: Vec<&str> = [
        ("identity", layout.identity()),
        ("board", layout.board()),
        ("services", layout.services()),
        ("ledger", layout.ledger()),
        ("storage", layout.storage()),
        ("catalog", layout.catalog()),
    ]
    .iter()
    .filter(|(_, path)| !path.exists())
    .map(|(name, _)| *name)
    .collect();
    // Provider storage is absent by design on a requester-only node.
    let missing: Vec<&str> = missing
        .into_iter()
        .filter(|name| {
            !(*name == "storage" && config.as_ref().is_some_and(|c| c.offering.is_none()))
        })
        .collect();
    if let (Some(text), Some(_)) = (identity.as_deref(), &config)
        && let Ok(local) = text::parse(text)
        && let Ok(catalog) = super::catalog::FileCatalog::open(&layout.catalog(), local)
    {
        lines.push(format!("leases: {}", catalog.list().len()));
    }
    lines.push(if missing.is_empty() && config.is_some() {
        "state: initialized".to_string()
    } else {
        format!("state: incomplete (missing: {})", missing.join(", "))
    });
    lines.push(format!(
        "control socket: {}",
        if layout.control().exists() {
            "present"
        } else {
            "absent"
        }
    ));
    Ok(lines.join("\n") + "\n")
}

// Create the root owner-only, or accept an existing empty directory; anything else is refused untouched.
fn prepare_root(root: &Path) -> Result<(), String> {
    tracing::trace!("Preparing the node root directory");
    match std::fs::read_dir(root) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return Err("node root already contains state and is left unchanged".into());
            }
            std::fs::set_permissions(root, std::os::unix::fs::PermissionsExt::from_mode(0o700))
                .map_err(|_| "node root permissions could not be set".to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => std::fs::DirBuilder::new()
            .mode(0o700)
            .create(root)
            .map_err(|_| "node root could not be created; its parent must exist".to_string()),
        Err(_) => Err("node root is not a readable directory".into()),
    }
}

// Create the protected identity with the same limits `identity-init` uses.
fn create_identity(path: &Path) -> Result<IdentityId, String> {
    tracing::trace!("Creating the node identity");
    let limits = Limits::new(1024, 64, 65548, 65536, 65536).map_err(|e| e.to_string())?;
    let (store, admin) = FileStore::open(path, limits).map_err(|e| e.to_string())?;
    let mut provider = Identity::new(store, SystemEntropy, limits, 1).map_err(|e| e.to_string())?;
    let (_, public) = provider.create(&admin).map_err(|e| e.to_string())?;
    Ok(public.identity.id())
}

// Write a new owner-only file; an existing file is never replaced.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    tracing::trace!("Writing a private node file");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "a node file could not be created".to_string())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "a node file could not be written".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    // A unique path below the temporary directory, not yet created.
    fn fresh() -> std::path::PathBuf {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        std::env::temp_dir().join(format!("node-init-{}", u64::from_le_bytes(nonce)))
    }
    // Fresh initialization creates every owner, an owner-only root and an inspectable summary.
    #[test]
    fn initializes_a_fresh_root_and_describes_it() {
        use std::os::unix::fs::PermissionsExt;
        let root = fresh();
        let id = initialize(&root, None).unwrap();
        assert_eq!(
            std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let shown = describe(&root).unwrap();
        assert!(shown.contains(&text::format(id)), "{shown}");
        assert!(
            shown.contains("state: initialized") && shown.contains("leases: 0"),
            "{shown}"
        );
        assert!(shown.contains("provider and requester"));
        assert!(shown.contains("control socket: absent"));
    }
    // A requester-only configuration initializes without Storage and still reads as initialized.
    #[test]
    fn requester_only_root_has_no_storage_and_is_complete() {
        let root = fresh();
        initialize(&root, Some(br#"{"version":1,"offering":null}"#)).unwrap();
        assert!(!Layout::new(&root).storage().exists());
        let shown = describe(&root).unwrap();
        assert!(
            shown.contains("requester only") && shown.contains("state: initialized"),
            "{shown}"
        );
    }
    // Existing state is never replaced, and an invalid configuration fails before creating anything.
    #[test]
    fn refuses_existing_state_and_invalid_configuration_without_effects() {
        let root = fresh();
        initialize(&root, None).unwrap();
        let before = describe(&root).unwrap();
        assert!(
            initialize(&root, None)
                .unwrap_err()
                .contains("already contains state")
        );
        assert_eq!(describe(&root).unwrap(), before);
        let other = fresh();
        assert!(initialize(&other, Some(b"{}")).is_err());
        assert!(!other.exists());
    }
    // A missing parent or a plain file as root fails explicitly.
    #[test]
    fn unusable_roots_are_refused() {
        let missing = fresh().join("child");
        assert!(initialize(&missing, None).is_err());
        let file = fresh();
        std::fs::write(&file, b"x").unwrap();
        assert!(initialize(&file, None).is_err());
        assert!(describe(&file).is_err());
        assert!(describe(&fresh()).is_err());
    }
    // A partly created root is reported as incomplete instead of failing inspection.
    #[test]
    fn partial_roots_are_inspectable() {
        let root = fresh();
        std::fs::create_dir(&root).unwrap();
        let shown = describe(&root).unwrap();
        assert!(shown.contains("state: incomplete"), "{shown}");
        assert!(shown.contains("identity: unknown"));
    }
    // Configuration files are bounded.
    #[test]
    fn configuration_files_are_bounded() {
        let root = fresh();
        std::fs::create_dir(&root).unwrap();
        let big = root.join("big.json");
        std::fs::write(&big, vec![b' '; MAXIMUM_DOCUMENT_BYTES + 1]).unwrap();
        assert!(read_document(&big).is_err());
        assert!(read_document(&root.join("missing.json")).is_err());
        std::fs::write(&big, b"{}").unwrap();
        assert_eq!(read_document(&big).unwrap(), b"{}");
    }
}
