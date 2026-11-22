//! Replicas of data the node hosts, kept at independent peers.
//!
//! Callers: node composition and the wanted work source. See [the module guide](mod.md).
pub mod book;
pub mod maintain;
pub mod report;
pub mod sealer;
