//! Explicit family ports for Services; handlers own resources and never receive Ledger internals.
use super::{Error, contract::OperationKey, value::Value};
use crate::modules::{
    board::{definition::Definition, offering::Offering},
    identity::api::IdentityId,
};
use std::sync::Arc;

/// Requester-calculated check conclusion; no automatic accounting or trust effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    /// Evidence satisfies the requested property.
    Pass,
    /// Complete evidence contradicts the property.
    Fail,
    /// Missing evidence, timing or local resources prevent a conclusion.
    Inconclusive,
}
/// Immutable owner-retained bytes; no session lifetime, path or process pointer is exposed.
#[derive(Clone, PartialEq, Eq)]
pub struct Blob(Arc<[u8]>);
impl Blob {
    /// Retain complete bytes only after checking the configured byte bound.
    /// # Errors
    /// Limit rejects oversized content without making an extra copy.
    pub fn new(bytes: Vec<u8>, maximum: usize) -> Result<Self, Error> {
        tracing::trace!("Retaining bounded immutable service bytes");
        if bytes.len() > maximum {
            return Err(Error::Limit);
        }
        Ok(Self(bytes.into()))
    }
    /// Borrow retained immutable complete bytes for this owner's lifetime.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing retained service bytes");
        &self.0
    }
}
/// Trusted authenticated context; composition constructs this after checking the channel.
#[derive(Clone, Copy)]
pub struct Peer {
    /// Identity served by this local runtime.
    pub local: IdentityId,
    /// Authenticated opposite endpoint, independent of payer/provider role.
    pub remote: IdentityId,
    /// Original accepted attempt association.
    pub key: OperationKey,
}
/// Bounded assessment output; no execution or settlement is authorized by assessment alone.
pub struct Assessment {
    /// Family-validated metadata used by the quote calculation.
    pub metadata: Value,
    /// Explicit provider size/factor values retained in the accepted quote.
    pub factors: Value,
    /// Maximum complete input bytes accepted by this assessment.
    pub input_bytes: u64,
    /// Optional owned pin; dropping the final reference releases provisional assessment resources.
    pub pin: Option<Blob>,
}
/// Exact immutable execution context retained by the coordinator before dispatch.
#[derive(Clone)]
pub struct Invocation {
    /// Bound requester/connection/stream instance.
    pub key: OperationKey,
    /// Application requester, independent of socket direction.
    pub requester: IdentityId,
    /// Application provider responsible for effects.
    pub provider: IdentityId,
    /// Exact accepted contract and quote bytes.
    pub accepted: Blob,
    /// Accepted fixed price or maximum metered price.
    pub maximum: u64,
}
/// Complete staged output, invisible as successful work until coordinator commitment.
#[derive(Clone)]
pub struct Completion {
    /// Logical result subject to schema and family validation.
    pub output: Value,
    /// Complete immutable result bytes retained independently of caller sessions.
    pub result: Blob,
    /// Exact final charge, validated against the accepted pricing contract.
    pub price: u64,
    /// Family-owned immutable reference to its staged durable effect.
    pub effect: Blob,
}
/// Durable participant outcome, queried instead of repeating execution after reply loss.
#[derive(Clone, PartialEq, Eq)]
pub enum Status {
    /// No staged operation is known.
    Absent,
    /// Exact effect is prepared with this participant-owned opaque token.
    Prepared(Blob),
    /// Effect committed once; complete result remains retained.
    Committed,
    /// Explicit coordinator abort has released staged resources.
    Aborted,
}
/// Requester-side codec and verifier independent of a provider installation.
pub trait Requester: super::validation::Semantics {
    /// Describe the exact installed contract; remote schemas cannot replace it.
    fn definition(&self) -> &Definition;
    /// Validate family-specific economic terms without spending or performing work.
    /// # Errors
    /// Reject unsupported pricing and invalid or overflowing terms.
    fn terms(&self, offering: &Offering) -> Result<(), Error>;
    /// Verify original input, agreed terms, complete evidence and local expectations.
    /// # Errors
    /// Return a typed dependency/validation failure; missing expectations should yield Inconclusive.
    fn verify(
        &self,
        input: &Value,
        terms: &Blob,
        result: &Value,
        expectations: Option<&Value>,
    ) -> Result<Check, Error>;
}
/// Provider port supplied only by authorized composition; no sockets, Board tables or Ledger access.
pub trait Provider: Send + Sync {
    /// Assess bounded metadata, authorization and capacity; never execute or settle.
    /// # Errors
    /// Refuse unsupported/invalid/capacity-limited work before acceptance.
    fn assess(
        &self,
        peer: Peer,
        offering: &Offering,
        metadata: &Value,
    ) -> Result<Assessment, Error>;
    /// Execute once after reservation and complete input validation; stage recoverable effects.
    /// # Errors
    /// Explicit uncommitted refusal differs from Unknown after possible external effects.
    fn execute(&self, invocation: &Invocation, input: &Value) -> Result<Completion, Error>;
    /// Recover staged completion without executing again.
    /// # Errors
    /// Unknown means no reliable completion can presently be established.
    fn completion(&self, key: OperationKey) -> Result<Completion, Error>;
    /// Durably prepare the exact staged effect and return an owner/key/effect-bound token.
    /// # Errors
    /// Changed effects conflict; dependency loss preserves uncertainty.
    fn prepare(&self, key: OperationKey, effect: &Blob) -> Result<Blob, Error>;
    /// Apply a durable coordinator commit at most once using the exact prepared token.
    /// # Errors
    /// Foreign tokens or contradictory decisions fail without changing the recorded decision.
    fn commit(&self, key: OperationKey, token: &Blob) -> Result<(), Error>;
    /// Release uncommitted resources only on an explicit recorded coordinator abort.
    /// # Errors
    /// Committed effects cannot be aborted; prepared effects require their exact token.
    fn abort(&self, key: OperationKey, token: Option<&Blob>) -> Result<(), Error>;
    /// Observe durable participant state without guessing rollback from timeout.
    /// # Errors
    /// Unavailable and Unknown retain their meaning across restart.
    fn status(&self, key: OperationKey) -> Result<Status, Error>;
    /// Return only contract-defined evidence for an independently authorized check.
    /// # Errors
    /// Local-only checks return Unsupported; absent entitlement or spending returns Denied.
    fn evidence(&self, invocation: &Invocation, challenge: &Value) -> Result<Value, Error>;
}
