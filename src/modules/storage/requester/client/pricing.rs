//! Exact Storage quotes use current accepted terms plus provider- and attempt-scoped independent lease snapshots.
use super::{
    Client,
    input::{kind, number, roles},
};
use crate::modules::{
    board::offering::{Direction, Offering},
    services::{Error, handler::Peer, pricing::Price, value::Value},
    storage::{
        assessment,
        contract::State,
        operation::{self, Operation},
        pricing, renewal,
        terms::Terms,
    },
};
impl Client {
    // Price requester-known dimensions and independently retained provider-scoped size; no body execution, catalog mutation or spending occurs.
    pub(super) fn price(
        &self,
        peer: Peer,
        offering: &Offering,
        metadata: &Value,
        factors: &Value,
    ) -> Result<Price, Error> {
        tracing::trace!(
            "Pricing Storage quote against original provider-scoped requester expectations"
        );
        let (requester, provider) = roles(peer)?;
        let terms = Terms::parse(offering)?;
        metadata.bounded(self.policy.values)?;
        factors.bounded(self.policy.values)?;
        let operation = kind(metadata)?;
        if operation == Operation::Store {
            return assessment::store_price(offering, metadata, factors);
        }
        operation::input(metadata, self.policy.values)?;
        if offering.direction != Direction::Provided {
            return Err(Error::Denied);
        }
        let lease = self.original(peer.key, provider, requester, renewal::id(metadata)?)?;
        if lease.state != State::Active {
            return Err(Error::Unavailable);
        }
        match operation {
            Operation::Store => Err(Error::Invalid),
            Operation::Renew => {
                renewal::price(offering, metadata, factors, &lease, self.policy.values)
            }
            Operation::Remove | Operation::Retrieve => {
                if !matches!(factors,Value::Object(fields) if fields.is_empty()) {
                    return Err(Error::Invalid);
                }
                if operation == Operation::Remove {
                    return Ok(Price::Fixed(terms.remove_price));
                }
                let offset = number(metadata, "offset")?;
                let length = number(metadata, "length")?;
                if offset >= lease.size_bytes || length > lease.size_bytes - offset {
                    return Err(Error::Invalid);
                }
                if length > terms.maximum_retrieve_bytes {
                    return Err(Error::Limit);
                }
                Ok(Price::Fixed(pricing::usage(
                    length,
                    1,
                    terms.retrieval,
                    terms.minimum_price,
                )?))
            }
        }
    }
}
