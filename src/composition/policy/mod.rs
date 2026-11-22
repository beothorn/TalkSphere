//! Peer policy: the decisions CR2SE leaves to each identity, isolated behind contracts.
//!
//! Callers: node composition. See [the module guide](mod.md) and the
//! [peer policy plan](../../../plans/12_Peer_policy/PLAN.md).
pub mod contracts;
pub mod defaults;

pub use contracts::{
    Admission, Candidate, Connection, Credit, Event, Grant, Limits, PeerView, Policy, Quota,
    Reason, Redundancy, Summary, Trust, Verdict, Wanted,
};
pub use defaults::Defaults;
