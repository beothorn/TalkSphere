//! Hosted publication requester intents for the Peer Protocol endpoint.
//!
//! Callers: a node's client side. The endpoint already validates quotes, reserves and commits exactly once, receives results only on the original
//! stream and supports family-authorized retransmission; what this module adds is the typed intent. [`Remote`] holds the selection of one host's
//! hosted publication offering and builds the endpoint plan for each operation with an *exact* spending bound: the price the requester itself calculates
//! from the offering and its own knowledge, so a quote above that price is refused before anything is reserved. A get is bounded by the content size the
//! reader expects (from a list entry) and a renewal by the size the publisher knows. Removals may be retried over a new stream only through
//! [`Remote::retry`], which reuses the original operation and never creates a new charge.
//! Dependencies: terms and pricing, the codec and the endpoint's public request contract.
use super::{
    codec::{self, Request},
    error::Error,
    operation::{CheckRequest, GetRequest, ListRequest, RemoveRequest, RenewRequest, StoreRequest},
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

/// One host's hosted publication offering, ready to turn operations into endpoint plans.
pub struct Remote {
    terms: Terms,
    selection: Selection,
    cache: Arc<PeerCache>,
}

impl Remote {
    /// A remote for the host whose direct metadata produced `selection`, with the terms validated from its offering.
    pub fn new(terms: Terms, selection: Selection, cache: Arc<PeerCache>) -> Self {
        tracing::trace!("Creating a hosted publication remote");
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

    /// Store a publication. The ceiling is the exact store price for its size and retention.
    /// # Errors
    /// `Invalid` when the size or retention is outside the offering; `Limit` when the price would overflow.
    pub fn store(&self, store: &StoreRequest) -> Result<(Plan, u64), Error> {
        tracing::trace!("Planning a remote store");
        let t = &self.terms;
        let size = store.publication.metadata.content_size;
        if size == 0
            || size > t.maximum_content_bytes
            || store.retention_seconds < t.minimum_retention_seconds
            || store.retention_seconds > t.maximum_retention_seconds
        {
            return Err(Error::Invalid);
        }
        let price = t.store_price(size, store.retention_seconds)?;
        Ok((
            self.invoke(codec::request_value(&Request::Store(store.clone())), price),
            price,
        ))
    }

    /// List a publisher's publications at the fixed list price.
    pub fn list(&self, request: ListRequest) -> (Plan, u64) {
        tracing::trace!("Planning a remote list");
        let price = self.terms.list_price;
        (
            self.invoke(codec::request_value(&Request::List(request)), price),
            price,
        )
    }

    /// Get a publication whose content size the reader learned from a list entry. The ceiling is the exact get price for that size, so a host
    /// that quotes a larger size is refused before anything is reserved.
    /// # Errors
    /// `Invalid` for a zero size or a size above `maximum_bytes`; `Limit` when the price would overflow.
    pub fn get(&self, request: GetRequest, expected_size: u64) -> Result<(Plan, u64), Error> {
        tracing::trace!("Planning a remote get");
        if expected_size == 0 || expected_size > request.maximum_bytes {
            return Err(Error::Invalid);
        }
        let price = self.terms.get_price(expected_size)?;
        Ok((
            self.invoke(codec::request_value(&Request::Get(request)), price),
            price,
        ))
    }

    /// Renew a placement whose content size the publisher knows.
    /// # Errors
    /// `Invalid` for a zero size or duration; `Limit` when the price would overflow.
    pub fn renew(&self, request: RenewRequest, content_size: u64) -> Result<(Plan, u64), Error> {
        tracing::trace!("Planning a remote renewal");
        let price = self
            .terms
            .renew_price(content_size, request.additional_seconds)?;
        Ok((
            self.invoke(codec::request_value(&Request::Renew(request)), price),
            price,
        ))
    }

    /// Remove a placement at the fixed removal price; the release credits the host quotes must meet the request's minimum.
    pub fn remove(&self, request: RemoveRequest) -> (Plan, u64) {
        tracing::trace!("Planning a remote removal");
        let price = self.terms.remove_price;
        (
            self.invoke(codec::request_value(&Request::Remove(request)), price),
            price,
        )
    }

    /// Challenge a pending placement for one content byte at the fixed check price.
    pub fn check(&self, request: &CheckRequest) -> (Plan, u64) {
        tracing::trace!("Planning a remote byte check");
        let price = self.terms.check_price;
        (
            Plan::Check(Box::new(Challenge {
                selection: self.selection.clone(),
                cache: self.cache.clone(),
                challenge: codec::check_input_value(request),
                spending: Spending {
                    maximum: price,
                    payment: Default::default(),
                },
                consent: Arc::new(Ceiling(price)),
            })),
            price,
        )
    }

    /// Retransmit an original removal over a new stream without a new quote or charge.
    pub fn retry(original: OperationKey) -> Plan {
        tracing::trace!("Planning a cached hosted publication retry");
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
