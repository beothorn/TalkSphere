//! Private immutable check terms back public accepted capabilities without transmitting requester expectations.
use super::*;
use crate::modules::services::{
    authorization::Authorized,
    checks::{Agreement, Invocation, Terms},
    handler::Blob,
    runtime::Payment,
    value::Value,
};
pub(super) struct State {
    pub authorized: Authorized,
    pub challenge: Value,
    pub bytes: Blob,
    pub hash: [u8; 32],
    pub id: [u8; 32],
    pub terms: Terms,
    pub maximum: u64,
    pub payment: Payment,
    pub expires: Instant,
    pub association: Association,
}
impl State {
    // Preserve all originally quoted terms in the narrow Services admission contract.
    pub fn agreement(self) -> Agreement {
        tracing::trace!("Binding immutable quoted check terms for Services admission");
        Agreement {
            authorized: self.authorized,
            challenge: self.challenge,
            bytes: self.bytes,
            hash: self.hash,
            maximum: self.maximum,
            payment: self.payment,
            terms: self.terms,
        }
    }
    // Build public check invocation context solely from installed roles and the immutable original quote.
    pub fn invocation(&self) -> Invocation {
        tracing::trace!("Constructing original public check invocation context");
        let selected = self.authorized.selection();
        let (requester, provider, issuer) = selected.roles();
        Invocation {
            key: self.authorized.key(),
            requester,
            provider,
            issuer,
            offering: selected.offering().clone(),
            definition: selected.definition().clone(),
            quote: self.bytes.clone(),
            price: self.terms.price,
            response_milliseconds: self.terms.response_milliseconds,
            deadline: self.expires,
        }
    }
}
/// Reserved original provider check; only its input terminal gate may dispatch evidence.
pub struct Incoming {
    pub(super) association: Association,
    pub(super) invocation: Invocation,
    pub(super) challenge: Value,
    pub(super) schema: serde_json::Value,
}
impl Incoming {
    /// Read original authenticated requester and stream correlation.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading accepted check provider association");
        self.association
    }
    /// Borrow the original immutable check economics and contract.
    pub fn invocation(&self) -> &Invocation {
        tracing::trace!("Borrowing accepted check provider invocation");
        &self.invocation
    }
}
/// Requester check reserved and durably marked dispatched before acceptance bytes become available.
pub struct Outgoing {
    pub(super) association: Association,
    pub(super) invocation: Invocation,
    pub(super) challenge: Value,
    pub(super) accept: Vec<u8>,
    pub(super) schema: serde_json::Value,
}
impl Outgoing {
    /// Read original authenticated provider association.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading accepted check requester association");
        self.association
    }
    /// Borrow challenge-only original invocation context, with no private expected value.
    pub fn invocation(&self) -> &Invocation {
        tracing::trace!("Borrowing accepted check requester invocation");
        &self.invocation
    }
    /// Borrow accept JSON only after durable local reservation and once-only dispatch marking.
    pub fn accept(&self) -> &[u8] {
        tracing::trace!("Borrowing check accept after reservation");
        &self.accept
    }
    /// Borrow only the original defined challenge; expectations belong to later local verification.
    pub fn challenge(&self) -> &Value {
        tracing::trace!("Borrowing accepted challenge without expectations");
        &self.challenge
    }
}
