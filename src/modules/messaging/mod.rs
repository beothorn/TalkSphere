//! Messaging: paid retention of signed messages for one recipient (CR2SE version 1).
//!
//! Callers: node composition and Services registration. See [the module guide](mod.md).
pub mod error;
pub mod ids;
pub mod metadata;
pub mod operation;
pub mod ports;
pub mod state;

pub use error::Error;

#[cfg(test)]
pub(crate) mod test_support;

pub mod standard;

#[cfg(test)]
mod fixture_tests;

pub mod pricing;

pub mod terms;

pub mod codec;

pub mod signing;

pub mod receipt;

pub mod encryption;

pub mod credential;
