//! Paths of one node root; separate protected directories keep owners independent.
//!
//! Callers: initialization, owner opening and the control channel. No filesystem effect
//! happens when computing paths.
use std::path::{Path, PathBuf};

/// File names inside a node root; every owner gets its own directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    root: PathBuf,
}
impl Layout {
    /// Describe a node root without touching the filesystem.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        tracing::trace!("Describing a node root layout");
        Self { root: root.into() }
    }
    /// The node root directory itself.
    pub fn root(&self) -> &Path {
        tracing::trace!("Reading the node root");
        &self.root
    }
    /// Operator configuration document.
    pub fn config(&self) -> PathBuf {
        tracing::trace!("Reading the node configuration path");
        self.root.join("node.json")
    }
    /// Operator policy document; optional, defaults reproduce the minimum node.
    pub fn policy(&self) -> PathBuf {
        tracing::trace!("Reading the node policy path");
        self.root.join("policy.json")
    }
    /// Public identity text written at initialization so inspection never needs the store lock.
    pub fn public_id(&self) -> PathBuf {
        tracing::trace!("Reading the node public identity path");
        self.root.join("public-id")
    }
    /// Protected Identity store directory.
    pub fn identity(&self) -> PathBuf {
        tracing::trace!("Reading the node identity path");
        self.root.join("identity")
    }
    /// Board owner directory.
    pub fn board(&self) -> PathBuf {
        tracing::trace!("Reading the node Board path");
        self.root.join("board")
    }
    /// Services owner directory.
    pub fn services(&self) -> PathBuf {
        tracing::trace!("Reading the node Services path");
        self.root.join("services")
    }
    /// Ledger owner directory.
    pub fn ledger(&self) -> PathBuf {
        tracing::trace!("Reading the node Ledger path");
        self.root.join("ledger")
    }
    /// Storage owner directory.
    pub fn storage(&self) -> PathBuf {
        tracing::trace!("Reading the node Storage path");
        self.root.join("storage")
    }
    /// Requester lease catalog file.
    pub fn catalog(&self) -> PathBuf {
        tracing::trace!("Reading the node catalog path");
        self.root.join("catalog.json")
    }
    /// Recent control operation outcomes, including unknown ones.
    pub fn operations(&self) -> PathBuf {
        tracing::trace!("Reading the node operations journal path");
        self.root.join("operations.json")
    }
    /// Durable per-peer observations read by peer policy.
    pub fn observations(&self) -> PathBuf {
        tracing::trace!("Reading the node observations path");
        self.root.join("observations.json")
    }
    /// Directory of the replica book and the sealed bytes of hosted items.
    pub fn replicas(&self) -> PathBuf {
        tracing::trace!("Reading the node replicas path");
        self.root.join("replicas")
    }
    /// Owner-only data key that seals hosted items before they are replicated.
    pub fn replica_key(&self) -> PathBuf {
        tracing::trace!("Reading the node replica key path");
        self.root.join("replica.key")
    }
    /// Local control socket path.
    pub fn control(&self) -> PathBuf {
        tracing::trace!("Reading the node control socket path");
        self.root.join("control.sock")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Every owner path is distinct and below the root, so owners cannot share state.
    #[test]
    fn owner_paths_are_distinct_children_of_the_root() {
        let layout = Layout::new("/n");
        let paths = [
            layout.config(),
            layout.policy(),
            layout.observations(),
            layout.replicas(),
            layout.replica_key(),
            layout.public_id(),
            layout.identity(),
            layout.board(),
            layout.services(),
            layout.ledger(),
            layout.storage(),
            layout.catalog(),
            layout.control(),
            layout.operations(),
        ];
        let unique: std::collections::BTreeSet<_> = paths.iter().collect();
        assert_eq!(unique.len(), paths.len());
        assert!(paths.iter().all(|p| p.starts_with(layout.root())));
    }
}
