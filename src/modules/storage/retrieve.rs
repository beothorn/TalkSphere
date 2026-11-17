//! Timely authenticated range acceptance retains immutable bytes independently of later lease state.
use super::{
    bytes::{Range, Store},
    contract::{Lease, Time},
    image::Image,
    lease,
    operation::{self, Operation},
    pricing,
    terms::Terms,
};
use crate::modules::{
    board::offering::Direction,
    services::{
        Error,
        contract::OperationKey,
        handler::{Blob, Invocation},
        pricing::Price,
        validation,
        value::{Budget, Value},
    },
};

/// Accepted provider-owned range and immutable original terms; resource admission remains with the caller.
pub struct Pinned {
    key: OperationKey,
    accepted: Blob,
    lease: Lease,
    range: Range,
    offset: u64,
    price: u64,
}
impl Pinned {
    /// Return the trusted original accepted operation key without creating any remote retry authority.
    pub fn key(&self) -> OperationKey {
        tracing::trace!("Reading original accepted Storage retrieval scope");
        self.key
    }
    // Preserve exact original public accepted terms for local result binding, without decoding coordinator state.
    pub(crate) fn accepted_terms(&self) -> &Blob {
        tracing::trace!("Borrowing original accepted Storage retrieval binding");
        &self.accepted
    }
    /// Borrow the exact authorized range; its descriptor survives subsequent unlink and metadata changes.
    pub fn bytes(&self) -> &Range {
        tracing::trace!("Borrowing an accepted Storage retrieval range");
        &self.range
    }
    /// Borrow lease metadata captured at timely acceptance, including the whole-object hash.
    pub fn lease(&self) -> &Lease {
        tracing::trace!("Borrowing accepted Storage retrieval lease metadata");
        &self.lease
    }
    /// Return original zero-based object offset, not a mutable file cursor.
    pub fn offset(&self) -> u64 {
        tracing::trace!("Reading accepted Storage retrieval offset");
        self.offset
    }
    /// Return the exact accepted work charge, independently of issuer transfer fees.
    pub fn price(&self) -> u64 {
        tracing::trace!("Reading accepted Storage retrieval work price");
        self.price
    }
}

/// Accept a fully validated paid retrieval while the authenticated owner's lease is active.
/// Invoke only after Services quote acceptance/reservation and complete input, through bounded owner admission.
/// `image` and clock observations are trusted owner snapshots; the offering is the immutable accepted current selection.
/// # Errors
/// Denied collapses missing/foreign lease access; Unavailable rejects inactive/expired resources;
/// Invalid rejects shape, range or price mismatch; Limit bounds; platform/clock failures preserve their meaning.
pub fn accept(
    invocation: &Invocation,
    input: &Value,
    image: &Image,
    store: &Store,
    now: Time,
    reboot_elapsed: Option<u64>,
    budget: Budget,
) -> Result<Pinned, Error> {
    tracing::trace!("Accepting an authenticated Storage range under immutable paid terms");
    super::binding::check(invocation)?;
    if invocation.provider != image.provider
        || invocation.key.requester != invocation.requester
        || invocation.publisher != invocation.provider
        || invocation.offering.direction != Direction::Provided
        || invocation.offering.issuer != invocation.issuer
    {
        return Err(Error::Denied);
    }
    let terms = Terms::parse(&invocation.offering)?;
    validation::validate(
        input,
        &invocation.definition.document.value()["input"],
        budget,
        false,
    )?;
    if operation::input(input, budget)? != Operation::Retrieve {
        return Err(Error::Invalid);
    }
    if !matches!(&invocation.factors,Value::Object(f) if f.is_empty()) {
        return Err(Error::Invalid);
    }
    let id = fixed(input, "leaseId")?;
    let offset = number(input, "offset")?;
    let length = number(input, "length")?;
    let candidate = image.leases.iter().find(|l| l.id == id);
    let lease = lease::authorize(candidate, image.provider, invocation.requester, &id)?;
    let logical = image.anchor.advance(now, reboot_elapsed)?.unix_seconds;
    if lease.state != super::contract::State::Active || logical >= lease.expires_at {
        return Err(Error::Unavailable);
    }
    if length > terms.maximum_retrieve_bytes {
        return Err(Error::Limit);
    }
    if offset >= lease.size_bytes || length > lease.size_bytes - offset {
        return Err(Error::Invalid);
    }
    let price = pricing::usage(length, 1, terms.retrieval, terms.minimum_price)?;
    if invocation.pricing != Price::Fixed(price) || invocation.maximum != price {
        return Err(Error::Invalid);
    }
    let range = store
        .reopen(lease.id, lease.size_bytes)?
        .range(offset, length)?;
    Ok(Pinned {
        key: invocation.key,
        accepted: invocation.accepted.clone(),
        lease: lease.clone(),
        range,
        offset,
        price,
    })
}
// Inspect only exact bounded logical fields after the shared operation-specific validator.
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    tracing::trace!("Reading accepted Storage retrieval field");
    let Value::Object(f) = value else {
        return Err(Error::Invalid);
    };
    f.iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .ok_or(Error::Invalid)
}
// Preserve uint64 offsets and lengths without overflow-shaped or lossy host-number conversion.
fn number(value: &Value, name: &str) -> Result<u64, Error> {
    tracing::trace!("Reading exact Storage retrieval dimension");
    match field(value, name)? {
        Value::Uint64(n) => Ok(*n),
        _ => Err(Error::Invalid),
    }
}
// A lease identifier is a fixed binary value, never a path or bearer authorization token.
fn fixed(value: &Value, name: &str) -> Result<[u8; 32], Error> {
    tracing::trace!("Reading original Storage retrieval resource identity");
    match field(value, name)? {
        Value::Bytes(b) => b.as_slice().try_into().map_err(|_| Error::Invalid),
        _ => Err(Error::Invalid),
    }
}
#[cfg(test)]
#[path = "tests/retrieve.rs"]
mod tests;
