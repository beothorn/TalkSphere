//! Deterministic simulation of the peer policy against an unreliable client fleet.
//!
//! Callers: the `policy_sim` example and tests. See [the module guide](mod.md).
pub mod abuse;
pub mod rng;
pub mod scenario;
pub mod world;
