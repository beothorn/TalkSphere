//! Authorized local protocol registrations used by connection stream routing.
use super::super::{
    api::Protocol,
    codec::open::{Open, OpenError},
    handles::StreamOwner,
};

/// Registration rejection; no authority is issued on failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationError {
    /// Selection violates the OPEN contract.
    Invalid(OpenError),
    /// An owner already handles this exact identifier and version.
    Duplicate,
}

/// Trusted composition's protocol-to-owner map; peer bytes never register handlers.
#[derive(Default)]
pub struct Registry {
    entries: Vec<(Protocol, StreamOwner)>,
}

impl Registry {
    /// Register one valid protocol/version and return its unique owner capability.
    /// # Errors
    /// Rejects invalid selections and duplicate registrations without changing state.
    pub fn register(&mut self, protocol: Protocol) -> Result<StreamOwner, RegistrationError> {
        tracing::trace!(
            function = "Registry::register",
            "Registering a local protocol consumer"
        );
        let selection =
            Open::new(&protocol.id, protocol.version).map_err(RegistrationError::Invalid)?;
        if self.resolve(selection).is_some() {
            return Err(RegistrationError::Duplicate);
        }
        let owner = StreamOwner::new();
        self.entries.push((protocol, owner.clone()));
        Ok(owner)
    }

    /// Resolve a valid peer selection to local authority, or None if unsupported.
    pub fn resolve(&self, selection: Open<'_>) -> Option<&StreamOwner> {
        tracing::trace!(
            function = "Registry::resolve",
            "Resolving a protocol consumer"
        );
        self.entries
            .iter()
            .find(|(p, _)| p.id == selection.id() && p.version == selection.version())
            .map(|(_, owner)| owner)
    }

    /// Return the registered selection belonging to a caller's capability.
    pub fn protocol(&self, owner: &StreamOwner) -> Option<&Protocol> {
        tracing::trace!(
            function = "Registry::protocol",
            "Checking registered consumer authority"
        );
        self.entries
            .iter()
            .find(|(_, candidate)| candidate == owner)
            .map(|(p, _)| p)
    }
}
