//! Hosted publication bindings to Services: the requester client and the provider adapter (crash recovery is added by a later plan step).
//!
//! Callers: node composition, which registers the adapter with the Services registry, and tests. Dependencies: the provider engine, the
//! participant, the codec and the Services handler contracts.
pub mod client;
mod outputs;
pub mod placements;
mod project;
pub mod provider;
pub mod recovery;

pub use client::{Client, Placements};
pub use placements::StorePlacements;
pub use provider::ServiceProvider;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod coordinator_tests;
#[cfg(test)]
mod crash_tests;
