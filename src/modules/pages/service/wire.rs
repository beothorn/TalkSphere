//! Page requester intents for the Peer Protocol endpoint.
//!
//! Callers: a visitor's client side. [`Remote`] holds the selection of one publisher's Page offering and builds the endpoint plan for a `get` with an *exact*
//! spending bound: the offering's fixed price, so a quote above it is refused before anything is reserved. Every additional resource needs its own plan and
//! its own price; nothing returned by a page can authorize further spending.
//! Dependencies: the terms, the codec and the endpoint's public request contract.
use super::{
    codec::{self, GetRequest},
    terms::Terms,
};
use crate::modules::{
    board::selection::Selection,
    pages::error::Error,
    peer_protocol::{
        engine::endpoint::{
            cache::PeerCache,
            requests::{Ceiling, Invocation, Plan},
        },
        requester::Spending,
    },
};
use std::sync::Arc;

/// One publisher's Page offering, ready to turn a path into an endpoint plan.
pub struct Remote {
    terms: Terms,
    selection: Selection,
    cache: Arc<PeerCache>,
}

impl Remote {
    /// A remote for the publisher whose direct metadata produced `selection`, with the terms validated from its offering.
    pub fn new(terms: Terms, selection: Selection, cache: Arc<PeerCache>) -> Self {
        tracing::trace!("Creating a Page remote");
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

    /// Get one resource at the offering's fixed price.
    /// # Errors
    /// `Invalid` for a path above the offering's path limit or a byte bound that is zero or above the offering's resource limit.
    pub fn get(&self, request: &GetRequest) -> Result<(Plan, u64), Error> {
        tracing::trace!("Planning a remote Page get");
        if request.path.size() as u64 > self.terms.maximum_path_bytes
            || request.maximum_bytes == 0
            || request.maximum_bytes > self.terms.maximum_resource_bytes
        {
            return Err(Error::Invalid);
        }
        let price = self.terms.price;
        Ok((
            Plan::Invoke(Box::new(Invocation {
                selection: self.selection.clone(),
                cache: self.cache.clone(),
                input: codec::request_value(request),
                spending: Spending {
                    maximum: price,
                    payment: Default::default(),
                },
                consent: Arc::new(Ceiling(price)),
            })),
            price,
        ))
    }
}
