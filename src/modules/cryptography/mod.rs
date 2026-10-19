//! Cryptographic contracts for Identity, Transport and content codecs.
//!
//! Only validated local policy and interfaces exist at this stage; there is no
//! production primitive provider. Composition supplies secure entropy and opaque
//! authorized key capabilities. Identity owns persistence, while Transport owns
//! transcripts, directional counters and replay handling.

pub mod api;
pub mod config;
pub mod error;
pub mod ports;
pub mod primitives;
pub mod random;
pub mod secrets;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod test_support;
