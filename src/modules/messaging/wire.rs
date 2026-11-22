//! Messaging requester intents for the Peer Protocol endpoint.
//!
//! Callers: a node's client side. The endpoint already validates quotes, reserves and commits exactly once, receives results only on
//! the original stream and supports family-authorized retransmission; what Messaging adds is the typed intent. [`Remote`] holds the
//! selection of one provider's Messaging offering and builds the endpoint plan for each operation with an *exact* spending bound:
//! the price the requester itself calculates from the offering and its own knowledge, so a quote above that price is refused
//! before anything is reserved. Recoveries and removals may be retried over a new stream only through [`Remote::retry`], which
//! reuses the original operation and never creates a new charge.
//! Dependencies: terms and pricing, the codec and the endpoint's public request contract.
use super::{
    codec::{self, Request},
    error::Error,
    operation::{
        CheckInput, DiscoverInput, RecoverInput, RemoveInput, RenewInput, StatusInput, StoreInput,
    },
    pricing,
    terms::Terms,
};
use crate::modules::{
    board::selection::Selection,
    peer_protocol::{
        engine::endpoint::{
            cache::PeerCache,
            requests::{Ceiling, Challenge, Invocation, Plan},
        },
        requester::Spending,
    },
    services::{contract::OperationKey, value::Value},
};
use std::sync::Arc;

/// One provider's Messaging offering, ready to turn operations into endpoint plans.
pub struct Remote {
    terms: Terms,
    selection: Selection,
    cache: Arc<PeerCache>,
}

impl Remote {
    /// A remote for the provider whose direct metadata produced `selection`, with the terms validated from its offering.
    pub fn new(terms: Terms, selection: Selection, cache: Arc<PeerCache>) -> Self {
        tracing::trace!("Creating a Messaging remote");
        Self {
            terms,
            selection,
            cache,
        }
    }

    /// The terms this remote prices from.
    pub fn terms(&self) -> &Terms {
        &self.terms
    }

    /// Store a message. The ceiling is the exact store price for its size and retention.
    /// # Errors
    /// `Invalid` when the size or retention is outside the offering; `Limit` when the price would overflow.
    pub fn store(&self, store: &StoreInput) -> Result<(Plan, u64), Error> {
        tracing::trace!("Planning a remote store");
        let t = &self.terms;
        let size = store.message.metadata.payload_size;
        if size == 0
            || size > t.maximum_payload_bytes
            || store.retention_seconds < t.minimum_retention_seconds
            || store.retention_seconds > t.maximum_retention_seconds
        {
            return Err(Error::Invalid);
        }
        let price = pricing::store(
            size,
            store.retention_seconds,
            t.payload_usage,
            t.payload_storage,
            t.minimum_price,
        )?;
        Ok((
            self.invoke(codec::request_value(&Request::Store(store.clone())), price),
            price,
        ))
    }

    /// List the caller's pending placements at the fixed discover price.
    pub fn discover(&self, input: DiscoverInput) -> (Plan, u64) {
        tracing::trace!("Planning a remote discovery");
        let price = self.terms.discover_price;
        (
            self.invoke(codec::request_value(&Request::Discover(input)), price),
            price,
        )
    }

    /// Recover a message whose payload size the recipient learned from discovery.
    /// # Errors
    /// `Invalid` for a zero size; `Limit` when the price would overflow.
    pub fn recover(&self, input: RecoverInput, payload_size: u64) -> Result<(Plan, u64), Error> {
        tracing::trace!("Planning a remote recovery");
        let price = pricing::recover(
            payload_size,
            self.terms.recovery_usage,
            self.terms.minimum_price,
        )?;
        Ok((
            self.invoke(codec::request_value(&Request::Recover(input)), price),
            price,
        ))
    }

    /// Ask the status of a placement at the fixed status price.
    pub fn status(&self, input: StatusInput) -> (Plan, u64) {
        tracing::trace!("Planning a remote status");
        let price = self.terms.status_price;
        (
            self.invoke(codec::request_value(&Request::Status(input)), price),
            price,
        )
    }

    /// Renew a placement whose payload size the sender knows.
    /// # Errors
    /// `Invalid` for a zero size or duration; `Limit` when the price would overflow.
    pub fn renew(&self, input: RenewInput, payload_size: u64) -> Result<(Plan, u64), Error> {
        tracing::trace!("Planning a remote renewal");
        let price = pricing::renew(
            payload_size,
            input.additional_seconds,
            self.terms.renewal_storage,
            self.terms.minimum_price,
        )?;
        Ok((
            self.invoke(codec::request_value(&Request::Renew(input)), price),
            price,
        ))
    }

    /// Remove a placement at the fixed removal price; the release credits the provider quotes must meet the input's minimum.
    pub fn remove(&self, input: RemoveInput) -> (Plan, u64) {
        tracing::trace!("Planning a remote removal");
        let price = self.terms.remove_price;
        (
            self.invoke(codec::request_value(&Request::Remove(input)), price),
            price,
        )
    }

    /// Challenge a pending placement for one byte at the fixed check price.
    pub fn check(&self, input: &CheckInput) -> (Plan, u64) {
        tracing::trace!("Planning a remote byte check");
        let price = self.terms.check_price;
        (
            Plan::Check(Box::new(Challenge {
                selection: self.selection.clone(),
                cache: self.cache.clone(),
                challenge: codec::check_input_value(input),
                spending: Spending {
                    maximum: price,
                    payment: Default::default(),
                },
                consent: Arc::new(Ceiling(price)),
            })),
            price,
        )
    }

    /// Retransmit an original recovery or removal over a new stream without a new quote or charge.
    pub fn retry(original: OperationKey) -> Plan {
        tracing::trace!("Planning a cached Messaging retry");
        Plan::Retry(original)
    }

    // An invocation whose spending ceiling and consent are both exactly `price`.
    fn invoke(&self, input: Value, price: u64) -> Plan {
        Plan::Invoke(Box::new(Invocation {
            selection: self.selection.clone(),
            cache: self.cache.clone(),
            input,
            spending: Spending {
                maximum: price,
                payment: Default::default(),
            },
            consent: Arc::new(Ceiling(price)),
        }))
    }
}
