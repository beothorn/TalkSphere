//! Narrow family-approved Messaging retransmission contracts preserve original accepted work and payment keys.
use super::{
    contract::OperationKey,
    handler::{Completion, Invocation, Peer},
    runtime::Payment,
    value::Value,
};
use crate::modules::identity::api::IdentityId;
use std::sync::Weak;
/// Version-one operations whose family may explicitly authorize cached retransmission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Original Messaging recovery, identified by recoveryId.
    Recovery,
    /// Original Messaging removal, identified by removalId.
    Removal,
}
/// Family-owned replay identity; equality alone never bypasses original caller and complete-input validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Id {
    /// Exact allowed operation purpose.
    pub purpose: Purpose,
    /// Original32-byte placement identifier scoping this replay ID.
    pub placement: [u8; 32],
    /// Original32-byte recovery/removal identifier.
    pub bytes: [u8; 32],
}
/// Fully decoded original request fields supplied by a trusted authenticated wire adapter, without current-price authority.
pub struct Proposal {
    /// Exact service identifier; only Messaging version one is eligible.
    pub service: String,
    /// Exact requested version.
    pub version: u32,
    /// Original offering lookup ID, even if no longer published.
    pub offering: String,
    /// Original authenticated publication owner.
    pub publisher: IdentityId,
    /// Original currency issuer.
    pub issuer: IdentityId,
    /// Original total spending bound; no fresh spending is authorized here.
    pub maximum: u64,
    /// Original third-party tuple, if any.
    pub payment: Payment,
    /// Complete schema-bound logical request, including every defined original field.
    pub input: Value,
}
/// Provider-owner proof of family-approved original committed output on one newly authenticated stream.
pub struct Cached {
    pub(crate) invocation: Invocation,
    pub(crate) completion: Completion,
    pub(crate) peer: Peer,
}
impl Cached {
    /// Borrow immutable original accepted terms, never a new quote or changed price.
    pub fn invocation(&self) -> &Invocation {
        tracing::trace!("Borrowing original cached invocation");
        &self.invocation
    }
    /// Borrow identical already committed output; returning it performs no new financial effect.
    pub fn completion(&self) -> &Completion {
        tracing::trace!("Borrowing original family-approved cached output");
        &self.completion
    }
    /// Read trusted new-stream routing independent of the original financial operation key.
    pub fn peer(&self) -> Peer {
        tracing::trace!("Reading cached response authenticated new stream");
        self.peer
    }
}
/// Explicit requester permission to retransmit one original Messaging request over a new stream without new reservation.
pub struct Retry {
    pub(crate) domain: Weak<()>,
    pub(crate) original: OperationKey,
    pub(crate) peer: Peer,
    pub(crate) invocation: Invocation,
    pub(crate) input: Value,
    pub(crate) id: Id,
    pub(crate) maximum: u64,
    pub(crate) payment: Payment,
}
impl Retry {
    /// Read original financial/recovery key; payment is applied through this key at most once.
    pub fn original(&self) -> OperationKey {
        tracing::trace!("Reading original retry payment association");
        self.original
    }
    /// Read authenticated new-stream provider binding.
    pub fn peer(&self) -> Peer {
        tracing::trace!("Reading explicit retry authenticated stream");
        self.peer
    }
    /// Borrow exact original contract and quote rather than current terms.
    pub fn invocation(&self) -> &Invocation {
        tracing::trace!("Borrowing explicit retry original terms");
        &self.invocation
    }
    /// Borrow every original defined request field without allowing mutation or automatic work retry.
    pub fn input(&self) -> &Value {
        tracing::trace!("Borrowing explicit retry original input");
        &self.input
    }
    /// Read original total bound retained before the first acceptance.
    pub fn maximum(&self) -> u64 {
        tracing::trace!("Reading original retry spending bound");
        self.maximum
    }
    /// Read original payment tuple; a retry cannot choose another transfer.
    pub fn payment(&self) -> Payment {
        tracing::trace!("Reading original retry payment binding");
        self.payment
    }
}
