//! Persistent identity contracts for local callers; Cryptography owns algorithms and Transport owns sessions.

pub mod api;
pub mod config;
pub mod error;
pub mod ports;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod test_support;
