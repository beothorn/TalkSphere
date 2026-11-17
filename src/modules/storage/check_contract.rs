//! Private shared original check associations and exact separately paid terms, independent of platform byte storage.
use super::terms::Terms;
use crate::modules::{
    board::offering::{Direction, integer},
    services::{Error, checks::Invocation},
};
// Validate immutable public check associations and exact paid terms without decoding coordinator-private records.
pub(super) fn binding(
    invocation: &Invocation,
    local: crate::modules::identity::api::IdentityId,
) -> Result<(), Error> {
    tracing::trace!("Checking original separately paid Storage check association");
    let offering = &invocation.offering;
    let definition = invocation.definition.document.value();
    let terms = Terms::parse(offering)?;
    if offering.direction != Direction::Provided
        || invocation.provider != local
        || invocation.key.requester != invocation.requester
        || invocation.issuer != offering.issuer
    {
        return Err(Error::Denied);
    }
    if definition["id"].as_str() != Some(offering.id.as_str())
        || definition["service"].as_str() != Some(offering.service.as_str())
        || definition.get("serviceVersion").and_then(integer) != Some(u64::from(offering.version))
        || definition["description"].as_str() != Some(offering.description.as_str())
    {
        return Err(Error::Invalid);
    }
    if invocation.quote.bytes().is_empty()
        || invocation.price != terms.check_price
        || invocation.response_milliseconds
            != terms
                .check_response_seconds
                .checked_mul(1000)
                .ok_or(Error::Limit)?
    {
        return Err(Error::Invalid);
    }
    Ok(())
}
