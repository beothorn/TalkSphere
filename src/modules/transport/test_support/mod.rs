//! Deterministic dependencies for unit tests, excluded from ordinary builds.
//! Scripts must be exhausted explicitly; scripted crypto never proves security.

pub mod clock;
pub mod consumer;
pub mod crypto;
pub mod identity;
pub mod network;
pub mod script;
pub mod socket;
pub mod vectors;
pub mod wake;
