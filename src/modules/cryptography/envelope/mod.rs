//! Generic single-object identity encryption, separate from service-specific envelopes.

pub mod decrypt;
pub mod derivation;
pub mod encrypt;
pub mod header;
pub mod object;

#[cfg(test)]
#[path = "tests/lifecycle.rs"]
mod lifecycle_tests;
