//! Peer policy: the decisions CR2SE leaves to each identity, isolated behind contracts.
//!
//! Callers: node composition. See [the module guide](mod.md) and the
//! [peer policy plan](../../../plans/12_Peer_policy/PLAN.md).
pub mod configured;
pub mod contracts;
pub mod observations;
pub mod params;
pub mod rates;

pub use configured::Configured;
pub use contracts::{
    Admission, Candidate, Connection, Credit, Event, Grant, Limits, PeerView, Policy, Quota,
    Reason, Redundancy, Summary, Trust, Verdict, Wanted,
};
pub use observations::{Invalid, Kind, Observations};
pub use params::{Params, ParamsError};
pub use rates::Rates;
