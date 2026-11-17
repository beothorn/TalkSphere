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

// Hash an unambiguous local original invocation binding; this creates no wire/retry identifier.
#[cfg(target_os = "linux")]
pub(super) fn retained(
    invocation: &Invocation,
    input: &super::super::services::value::Value,
    limits: super::image::Limits,
) -> Result<[u8; 32], Error> {
    tracing::trace!("Binding retained Storage mutation to exact original accepted terms and input");
    use crate::modules::{cryptography::primitives::hash::Sha256State, services::codec};
    let encoded = codec::encode(input, limits.values, limits.bytes)?;
    let factors = codec::encode(&invocation.factors, limits.values, limits.bytes)?;
    let stream = invocation.key.stream.to_be_bytes();
    let maximum = invocation.maximum.to_be_bytes();
    let fields: [&[u8]; 12] = [
        b"TalkSphere Storage accepted mutation v1",
        &invocation.key.requester.0,
        &invocation.key.connection,
        &stream,
        &invocation.provider.0,
        &invocation.publisher.0,
        &invocation.issuer.0,
        &maximum,
        invocation.accepted.bytes(),
        invocation.quote.bytes(),
        &encoded,
        &factors,
    ];
    let size = fields.iter().try_fold(0usize, |n, b| {
        n.checked_add(8)
            .and_then(|n| n.checked_add(b.len()))
            .ok_or(Error::Limit)
    })?;
    if size > limits.bytes {
        return Err(Error::Limit);
    }
    let mut hash = Sha256State::new(size);
    for field in fields {
        hash.update(&(field.len() as u64).to_be_bytes())
            .map_err(|_| Error::Limit)?;
        hash.update(field).map_err(|_| Error::Limit)?;
    }
    Ok(hash.finish())
}
