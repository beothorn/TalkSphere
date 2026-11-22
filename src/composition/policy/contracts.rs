//! Policy contracts: every decision CR2SE leaves to local policy, as a pure function of recorded facts.
//!
//! Callers: node composition (admission, quotas, credits, connections) and the replica manager.
//! Dependencies: only identities and plain numbers, so another deployment can replace any
//! contract without touching protocol modules. A contract decides from a [`PeerView`] built
//! out of operator configuration, durable observations and the clock; it never reads
//! peer-supplied text and never changes a CR2SE price, wire value or failure code.
use crate::modules::identity::api::IdentityId;
use std::sync::Arc;

/// Bounded counters about one peer, produced by the observation store.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    /// Unix seconds when the peer was first seen, if ever.
    pub first_seen: Option<u64>,
    /// Authenticated connections accepted from this peer.
    pub connections: u32,
    /// Contributions this node verified (for example a stored object it later retrieved).
    pub contributions: u32,
    /// Paid checks this peer passed.
    pub checks_passed: u32,
    /// Paid checks this peer failed with matching negative evidence.
    pub checks_failed: u32,
    /// Requests this node refused for the peer by quota, rate or capacity.
    pub refusals: u32,
    /// Operations that ended in failure attributable to the peer.
    pub failures: u32,
    /// Credits issued to this peer by bootstrap policy.
    pub bootstrap_credits: u64,
}

/// Everything a decision may depend on for one peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerView {
    /// Authenticated identity of the peer.
    pub peer: IdentityId,
    /// Current local trust in millionths, `0..=1_000_000`.
    pub trust_millionths: u32,
    /// Recorded observations.
    pub summary: Summary,
    /// Current Unix time in seconds.
    pub now_seconds: u64,
}

/// Why a policy refused; stable names for logs and `node-ctl policy explain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The node holds as many live connections as its policy allows.
    ConnectionLimit,
    /// The peer already holds as many live connections as its policy allows.
    PeerConnectionLimit,
    /// The peer exceeded a request or failure rate.
    RateLimited,
    /// A finite budget is used up.
    BudgetExhausted,
    /// The decision is disabled by configuration.
    Disabled,
}

/// Outcome of an admission or quota decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Proceed.
    Allow,
    /// Refuse before any work, charge or reservation.
    Refuse(Reason),
}

/// Decides whether an authenticated connection stays open.
pub trait Admission: Send + Sync {
    /// `live` is the node's current connection count and `from_peer` the peer's own count,
    /// both excluding the connection being decided.
    fn admit(&self, peer: &PeerView, live: usize, from_peer: usize) -> Verdict;
}

/// Finite per-identity limits applied before a quote is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Simultaneous pending operations.
    pub pending: usize,
    /// Requests allowed in one rolling minute, including free control reads.
    pub requests_per_minute: u32,
    /// Failed authentication or probing attempts allowed in one rolling minute.
    pub failures_per_minute: u32,
}

/// Chooses the per-identity limits.
pub trait Quota: Send + Sync {
    /// Limits for `peer`; they may depend on trust and history.
    fn limits(&self, peer: &PeerView) -> Limits;
}

/// A verified observation that may change trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A paid check returned matching evidence in time.
    CheckPassed,
    /// A paid check returned non-matching evidence.
    CheckFailed,
    /// A contribution was completed and verified.
    ContributionVerified,
    /// An operation attributable to the peer failed.
    Failed,
}

/// Adjusts trust from verified observations only.
pub trait Trust: Send + Sync {
    /// New trust in millionths, or `None` to leave trust unchanged.
    fn adjust(&self, peer: &PeerView, event: Event) -> Option<u32>;
}

/// A finite first grant for a new identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grant {
    /// Credits issued by this node.
    pub credits: u64,
    /// Trust assigned together with the credits, in millionths.
    pub trust_millionths: u32,
}

/// Terms of the wanted Storage offering this node publishes to earn resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted {
    /// Credits issued per fulfilled object.
    pub credits_per_object: u64,
    /// Largest object this node asks for, in bytes.
    pub maximum_object_bytes: u64,
    /// Retention requested for each object, in seconds.
    pub retention_seconds: u64,
}

/// Chooses credit issuance.
pub trait Credit: Send + Sync {
    /// First grant for a peer that has none, given the total already issued by bootstrap.
    fn bootstrap(&self, peer: &PeerView, issued_total: u64) -> Option<Grant>;
    /// Wanted Storage terms to publish, or `None` to publish none.
    fn wanted(&self) -> Option<Wanted>;
    /// Early-release credits to offer for removing a placement; bounded by the caller.
    fn release(&self, cumulative_payments: u64, minimum: u64) -> u64;
}

/// A peer that could hold a replica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    /// Identity of the candidate.
    pub peer: IdentityId,
    /// Whether the node has a live connection to it now.
    pub connected: bool,
    /// Its recorded history.
    pub summary: Summary,
    /// Its current trust in millionths.
    pub trust_millionths: u32,
}

/// Chooses replica placement and cadence.
pub trait Redundancy: Send + Sync {
    /// Number of independent confirmed agreements wanted for each item.
    fn target(&self) -> usize;
    /// Pick up to `need` new holders among `candidates`, never one in `have`.
    fn select(&self, candidates: &[Candidate], have: &[IdentityId], need: usize)
    -> Vec<IdentityId>;
    /// Unix time of the next check for the `index`-th replica after `last_checked`.
    fn next_check(&self, last_checked: u64, index: usize) -> u64;
    /// Remaining retention, in seconds, at or below which a replica is renewed.
    fn renew_before_seconds(&self) -> u64;
    /// Most credits one check or renewal may cost.
    fn max_spend(&self) -> u64;
    /// Seconds to wait before retrying a check or renewal that could not be completed.
    fn retry_seconds(&self) -> u64;
}

/// Chooses connection upkeep.
pub trait Connection: Send + Sync {
    /// Seconds between pings on an idle connection.
    fn ping_interval_seconds(&self) -> u64;
    /// Seconds to wait for a pong before closing.
    fn ping_timeout_seconds(&self) -> u64;
    /// Seconds to wait before reconnect attempt number `attempt` (starting at 0).
    fn reconnect_delay_seconds(&self, attempt: u32) -> u64;
}

/// The six contracts a node is composed with.
#[derive(Clone)]
pub struct Policy {
    /// Connection admission.
    pub admission: Arc<dyn Admission>,
    /// Per-identity limits.
    pub quota: Arc<dyn Quota>,
    /// Trust adjustment.
    pub trust: Arc<dyn Trust>,
    /// Credit issuance.
    pub credit: Arc<dyn Credit>,
    /// Replica placement and cadence.
    pub redundancy: Arc<dyn Redundancy>,
    /// Connection upkeep.
    pub connection: Arc<dyn Connection>,
}
