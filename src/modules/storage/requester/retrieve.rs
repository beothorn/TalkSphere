//! Complete requester retrieval verification against retained lease metadata and optional private range expectations.
use crate::modules::{
    board::offering::Direction,
    cryptography::primitives::hash::sha256,
    services::{
        Error,
        handler::Invocation,
        pricing::Price,
        validation,
        value::{Budget, Value},
    },
    storage::{
        contract::Lease,
        operation::{self, Operation},
        pricing,
        terms::Terms,
    },
};

/// Borrowed complete result; metadata acceptance alone is not proof of partial-range contents.
/// The output owner must keep its bounded complete value alive for this borrow.
#[derive(Debug, PartialEq, Eq)]
pub struct Retrieved<'a> {
    /// Exact accepted range, exposed only after every applicable verification succeeds.
    pub data: &'a [u8],
    /// True only when these bytes comprise the complete object and match its independently retained SHA-256.
    pub whole_object_hash_checked: bool,
    /// True only when bytes match an independently supplied requester-owned range of exactly the requested length.
    pub requester_bytes_compared: bool,
}

/// Validate complete authenticated output against original accepted retrieval and independently retained lease metadata.
/// The caller admits complete successful Services output before calling; provisional stream fragments are not output.
/// `expected_range` is optional private original data, never a value asserted by the provider.
/// Later expiration/removal does not invalidate a timely accepted operation; no current clock is consulted here.
/// # Errors
/// Denied rejects inconsistent owner/provider roles; Invalid rejects malformed metadata, bytes, ranges or pricing;
/// Unsupported rejects another service/version, and Limit rejects configured value or arithmetic bounds.
pub fn validate<'a>(
    invocation: &Invocation,
    input: &Value,
    output: &'a Value,
    lease: &Lease,
    expected_range: Option<&[u8]>,
    budget: Budget,
) -> Result<Retrieved<'a>, Error> {
    tracing::trace!("Validating complete Storage retrieval against private requester expectations");
    super::super::binding::check(invocation)?;
    if invocation.offering.direction != Direction::Provided
        || invocation.publisher != invocation.provider
        || invocation.provider != lease.provider
        || invocation.requester != lease.requester
    {
        return Err(Error::Denied);
    }
    let definition = invocation.definition.document.value();
    validation::validate(input, &definition["input"], budget, false)?;
    validation::validate(output, &definition["output"], budget, false)?;
    if operation::input(input, budget)? != Operation::Retrieve {
        return Err(Error::Invalid);
    }
    operation::output(output, Operation::Retrieve, budget)?;
    let terms = Terms::parse(&invocation.offering)?;
    let offset = number(input, "offset")?;
    let length = number(input, "length")?;
    if bytes(input, "leaseId")? != lease.id
        || offset >= lease.size_bytes
        || length > lease.size_bytes - offset
        || length > terms.maximum_retrieve_bytes
        || !matches!(&invocation.factors, Value::Object(f) if f.is_empty())
    {
        return Err(Error::Invalid);
    }
    let price = pricing::usage(length, 1, terms.retrieval, terms.minimum_price)?;
    let data = bytes(output, "data")?;
    if invocation.pricing != Price::Fixed(price)
        || invocation.maximum != price
        || number(output, "price")? != price
        || bytes(output, "leaseId")? != lease.id
        || number(output, "offset")? != offset
        || u64::try_from(data.len()).map_err(|_| Error::Limit)? != length
        || bytes(output, "contentHash")? != lease.content_hash
    {
        return Err(Error::Invalid);
    }
    let whole = offset == 0 && length == lease.size_bytes;
    if whole && sha256(data, budget.bytes).map_err(|_| Error::Limit)? != lease.content_hash {
        return Err(Error::Invalid);
    }
    if expected_range.is_some_and(|expected| expected != data) {
        return Err(Error::Invalid);
    }
    Ok(Retrieved {
        data,
        whole_object_hash_checked: whole,
        requester_bytes_compared: expected_range.is_some(),
    })
}

// Borrow a mandatory field only after bounded validation rejects duplicate names and wrong object kinds.
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    tracing::trace!("Reading original requester retrieval field");
    let Value::Object(fields) = value else {
        return Err(Error::Invalid);
    };
    fields
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .ok_or(Error::Invalid)
}
// Preserve the full protocol uint64 range without lossy host or floating-point conversions.
fn number(value: &Value, name: &str) -> Result<u64, Error> {
    tracing::trace!("Reading exact requester retrieval dimension");
    match field(value, name)? {
        Value::Uint64(n) => Ok(*n),
        _ => Err(Error::Invalid),
    }
}
// Borrow byte data without copying potentially large payloads before validation finishes.
fn bytes<'a>(value: &'a Value, name: &str) -> Result<&'a [u8], Error> {
    tracing::trace!("Borrowing complete requester retrieval bytes");
    match field(value, name)? {
        Value::Bytes(b) => Ok(b),
        _ => Err(Error::Invalid),
    }
}

#[cfg(test)]
#[path = "../tests/retrieval_validation.rs"]
mod tests;
