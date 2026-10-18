//! Transport authentication constructions; primitive operations remain injected.
//! Role ordering and domain separation are protocol behavior, not Crypto policy.
mod keys;
mod state;
mod transcript;
pub use keys::{DirectionKeys, SessionKeys, derive_keys, nonce};
pub use state::{Established, Handshake, HandshakeError};
pub use transcript::Transcript;

#[cfg(test)]
mod tests;
