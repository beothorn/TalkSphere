//! Constant-space stream allocation history for a single authenticated connection.
//! The connection owner serializes allocation before making OPEN visible on the wire.
use crate::modules::transport::api::ConnectionRole;

/// Failure to allocate locally or accept exactly the next remote OPEN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdError {
    /// No further identifier of this endpoint's parity fits in 32 bits.
    Exhausted,
    /// Remote OPEN uses zero, wrong parity, a gap or a previously consumed ID.
    Unexpected,
}

/// Per-connection high-water marks; closed/refused IDs need no tombstone storage.
#[derive(Debug)]
pub struct StreamIds {
    local_odd: bool,
    next_local: Option<u32>,
    next_remote: Option<u32>,
    greatest_local: u32,
    greatest_remote: u32,
}

impl StreamIds {
    // Position an orchestration fixture after the final permitted local allocation.
    #[cfg(test)]
    pub(super) fn exhaust_local(&mut self) {
        self.next_local = None;
    }

    /// Start initiator allocation at one and acceptor allocation at two.
    pub fn new(role: ConnectionRole) -> Self {
        tracing::trace!(
            function = "StreamIds::new",
            "Starting connection stream numbering"
        );
        let local_odd = role == ConnectionRole::Initiator;
        Self {
            local_odd,
            next_local: Some(if local_odd { 1 } else { 2 }),
            next_remote: Some(if local_odd { 2 } else { 1 }),
            greatest_local: 0,
            greatest_remote: 0,
        }
    }

    /// Consume the next local ID after admission, before queueing its OPEN.
    /// # Errors
    /// Exhaustion requires draining this connection; allocation never wraps.
    pub fn allocate(&mut self) -> Result<u32, IdError> {
        tracing::trace!(
            function = "StreamIds::allocate",
            "Allocating the next local stream"
        );
        let id = self.next_local.ok_or(IdError::Exhausted)?;
        self.next_local = id.checked_add(2);
        self.greatest_local = id;
        Ok(id)
    }

    /// Consume exactly the next remote ID before parsing or refusing its payload.
    /// # Errors
    /// Any other ID is a connection violation and leaves history unchanged.
    pub fn accept(&mut self, id: u32) -> Result<(), IdError> {
        tracing::trace!(
            function = "StreamIds::accept",
            "Checking remote stream sequence"
        );
        if self.next_remote != Some(id) {
            return Err(IdError::Unexpected);
        }
        self.next_remote = id.checked_add(2);
        self.greatest_remote = id;
        Ok(())
    }

    /// Distinguish any previously created/refused ID from a never-created ID.
    /// The active-stream table separately distinguishes active from terminal IDs.
    pub fn was_created(&self, id: u32) -> bool {
        tracing::trace!(
            function = "StreamIds::was_created",
            "Checking stream history"
        );
        id != 0
            && id
                <= if self.is_local(id) {
                    self.greatest_local
                } else {
                    self.greatest_remote
                }
    }

    /// Determine which endpoint owns a nonzero ID's parity; zero is never local.
    pub fn is_local(&self, id: u32) -> bool {
        tracing::trace!(
            function = "StreamIds::is_local",
            "Checking stream creator parity"
        );
        id != 0 && (id % 2 == 1) == self.local_odd
    }

    /// Return the greatest allocated local ID, or zero before any local OPEN.
    pub fn greatest_local(&self) -> u32 {
        tracing::trace!(
            function = "StreamIds::greatest_local",
            "Reading local stream watermark"
        );
        self.greatest_local
    }

    /// Return the greatest consumed remote ID, including refused OPEN requests.
    pub fn greatest_remote(&self) -> u32 {
        tracing::trace!(
            function = "StreamIds::greatest_remote",
            "Reading remote stream watermark"
        );
        self.greatest_remote
    }
}

#[cfg(test)]
#[path = "ids/tests.rs"]
mod tests;
