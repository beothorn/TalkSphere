//! Shared original invocation association checks; installed standard semantics remain Registry-owned.
use crate::modules::{
    board::offering::{Direction, integer},
    services::{Error, handler::Invocation},
};

// Validate immutable public associations without decoding another owner's accepted/journal representation.
pub(super) fn check(invocation: &Invocation) -> Result<(), Error> {
    tracing::trace!("Checking original Storage invocation identity and contract association");
    let offering = &invocation.offering;
    let definition = invocation.definition.document.value();
    if offering.service != "cr2se.storage" || offering.version != 1 {
        return Err(Error::Unsupported);
    }
    if definition["service"].as_str() != Some(offering.service.as_str())
        || definition.get("serviceVersion").and_then(integer) != Some(u64::from(offering.version))
        || definition["id"].as_str() != Some(offering.id.as_str())
        || definition["description"].as_str() != Some(offering.description.as_str())
    {
        return Err(Error::Invalid);
    }
    let publisher = match offering.direction {
        Direction::Provided => invocation.provider,
        Direction::Wanted => invocation.requester,
    };
    if invocation.key.requester != invocation.requester
        || invocation.publisher != publisher
        || invocation.issuer != offering.issuer
    {
        return Err(Error::Denied);
    }
    if invocation.accepted.bytes().is_empty() {
        return Err(Error::Invalid);
    }
    Ok(())
}
