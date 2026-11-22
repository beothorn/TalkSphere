//! Services bindings for Messaging: the provider adapter and the requester client.
//!
//! Callers: application composition, which registers one [`client::Client`] (the requester contract and economic rules, shared
//! by both roles) and optionally one [`provider::ServiceProvider`] with the Services registry. This layer translates between
//! Services' logical values, staged completions and decisions and the Messaging engine; it holds no protocol rules of its own.
//! Dependencies: the provider engine, the participant, the codec and the Services handler contracts.
pub mod client;
pub mod provider;
pub mod recovery;
pub mod release;

mod outputs;
mod project;

#[cfg(test)]
mod crash;
#[cfg(test)]
mod tests;
