//! Hosted Publications: provider-hosted signed pages and feed posts (CR2SE proposal, service version 1).
//!
//! Callers: node composition, Services registration and the pages and feeds modules. See [the module guide](mod.md).
pub mod error;
pub mod ids;

pub use error::Error;

pub mod metadata;

pub mod signing;

pub mod verify;
