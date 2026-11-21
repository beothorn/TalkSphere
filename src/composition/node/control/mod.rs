//! Authorized local control of a running node over an owner-only Unix socket.
//!
//! Callers: the `node-ctl` CLI command (through [`client`]) and the node runtime (through
//! [`socket::start`]). Authority is the owner-only node root and socket permissions; the
//! channel is never reachable from the network and is not the CR2SE Node API.
pub mod client;
pub mod protocol;
pub mod socket;
