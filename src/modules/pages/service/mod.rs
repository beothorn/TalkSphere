//! The publisher-served Page service `cr2se.page` version 1: the offering terms, the typed `get`, the local check and the Services bindings.
//!
//! Callers: node composition, which registers the client (and, for a publisher, the provider) with the Services registry, and the viewer. The publisher's own
//! identity is the provider; a visitor retrieves one resource per paid `get`, and the check is local and included. See [the guide](mod.md).
pub mod check;
pub mod client;
pub mod codec;
pub mod provider;
pub mod standard;
pub mod store;
pub mod terms;

#[cfg(test)]
mod coordinator_tests;
#[cfg(test)]
mod tests;
