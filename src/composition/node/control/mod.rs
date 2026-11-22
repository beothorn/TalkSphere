//! Authorized local control of a running node over an owner-only Unix socket.
//!
//! Callers: the `node-ctl` CLI command (through [`client`]) and the node runtime (through
//! [`socket::start`]). Authority is the owner-only node root and socket permissions; the
//! channel is never reachable from the network and is not the CR2SE Node API.
pub mod client;
pub mod journal;
pub mod protocol;

pub mod commands;
pub mod socket;

use crate::composition::node::{layout::Layout, replicas::maintain::Maintenance, runtime::Started};
use std::sync::Arc;

/// Everything the control channel keeps running; dropping it stops both parts.
pub struct Running {
    /// The local control socket.
    pub server: socket::Server,
    /// Replica checks and renewals.
    pub maintenance: Maintenance,
}

/// Start the control channel for a ready node.
/// # Errors
/// The operations journal is malformed or the socket cannot be bound safely.
pub fn start(layout: &Layout, started: &Started) -> Result<Running, String> {
    tracing::trace!("Starting local control for a ready node");
    let handler = Arc::new(commands::Handler::new(
        started.owners.clone(),
        started.link.clone(),
        started.signal.clone(),
        started.listen,
        &layout.operations(),
    )?);
    let server = socket::start(
        layout,
        started.owners.config.control,
        started.signal.clone(),
        handler.clone(),
    )?;
    let maintenance = Maintenance::start(move |attempts| handler.maintain_once(attempts));
    Ok(Running {
        server,
        maintenance,
    })
}
