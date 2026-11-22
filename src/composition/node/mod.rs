//! Minimum useful node composition: durable owners, Peer Protocol, paid Storage and local control.
//!
//! Callers are the `node-*` and `serve --node-root` CLI commands. Dependencies are the
//! public contracts of Identity, Board, Ledger, Services, Peer Protocol and Storage; no
//! module's private state is used. The ordinary `serve` composition is unchanged.
pub mod adapters;
pub mod catalog;
pub mod config;
pub mod control;
pub mod init;
pub mod layout;
pub mod link;
pub mod observation_store;
pub mod offering;
pub mod owners;
pub mod peers;
pub mod replicas;
pub mod runtime;
pub mod upkeep;
pub mod wanted;
