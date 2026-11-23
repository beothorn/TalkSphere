//! Hosted publication bindings to Services: the provider adapter (and, in later steps, the requester client and recovery).
//!
//! Callers: node composition, which registers the adapter with the Services registry, and tests. Dependencies: the provider engine, the
//! participant, the codec and the Services handler contracts.
mod outputs;
mod project;
pub mod provider;

pub use provider::ServiceProvider;

#[cfg(test)]
mod tests;
