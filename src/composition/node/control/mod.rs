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

use crate::composition::node::{layout::Layout, runtime::Started};
use std::sync::Arc;

/// Start the control channel for a ready node.
/// # Errors
/// The operations journal is malformed or the socket cannot be bound safely.
pub fn start(layout: &Layout, started: &Started) -> Result<socket::Server, String> {
    tracing::trace!("Starting local control for a ready node");
    let handler = commands::Handler::new(
        started.owners.clone(),
        started.link.clone(),
        started.signal.clone(),
        started.listen,
        &layout.operations(),
    )?;
    socket::start(
        layout,
        started.owners.config.control,
        started.signal.clone(),
        Arc::new(handler),
    )
}
