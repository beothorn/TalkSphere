//! Cryptographic contracts for Identity, Transport and content codecs.
//!
//! Implemented operations are listed in the module README and detailed plan.
//! Composition supplies secure entropy and opaque authorized key capabilities.
//! Identity owns persistence; Transport owns transcripts, counters and replay.

pub mod api;
pub mod config;
pub mod credential;
pub mod envelope;
pub mod error;
pub mod ports;
pub mod primitives;
pub mod provider;
pub mod random;
pub mod secrets;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod test_support;
