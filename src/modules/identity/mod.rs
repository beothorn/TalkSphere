//! Persistent identity contracts for local callers; Cryptography owns algorithms and Transport owns sessions.

pub mod api;
pub mod authority;
pub mod config;
pub mod contact;
pub mod contacts;
pub mod crypto;
pub mod error;
pub mod id;
pub mod ports;
pub mod record;
pub mod text;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod test_support;
