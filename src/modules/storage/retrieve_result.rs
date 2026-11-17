//! Complete verified range output and original-key durable retention without reexecuting committed reads.
use super::{effect::Effect, image, journal::Journal, quota::Usage, retrieve::Pinned};
use crate::modules::{
    cryptography::primitives::hash::Sha256State,
    services::{
        self, Error,
        handler::{Blob, Completion},
        value::{Budget, Value},
    },
};

/// Finite result and internal integrity-scan policy supplied by owner composition.
#[derive(Clone, Copy)]
pub struct Limits {
    /// Maximum complete local encoded result bytes, including metadata overhead.
    pub result_bytes: usize,
    /// Maximum whole-object bytes inspected to ensure the returned range is original immutable content.
    pub object_bytes: u64,
    /// Independent complete logical output value budgets.
    pub values: Budget,
}
// Exact overhead of the current public Services local value codec for the fixed retrieve result fields.
const ENCODED_OVERHEAD: u64 = 181;
// Field names, operation text, lease ID and whole-object hash occupy these logical bytes before range data.
const LOGICAL_OVERHEAD: u64 = 114;

/// Calculate a conservative original-key result hold before allocating or reading the accepted body.
/// # Errors
/// Limit rejects arithmetic/address-space or configured output/scan/structural bounds.
pub fn reservation(pin: &Pinned, limits: Limits) -> Result<Usage, Error> {
    tracing::trace!("Bounding complete Storage retrieval result capacity before I/O");
    let length = pin.bytes().len();
    let encoded = length.checked_add(ENCODED_OVERHEAD).ok_or(Error::Limit)?;
    let logical = length.checked_add(LOGICAL_OVERHEAD).ok_or(Error::Limit)?;
    if encoded > limits.result_bytes as u64
        || logical > limits.values.bytes as u64
        || limits.values.nodes < 7
        || limits.values.depth < 1
        || pin.lease().size_bytes > limits.object_bytes
    {
        return Err(Error::Limit);
    }
    Ok(Usage {
        operations: 1,
        result_bytes: encoded.checked_mul(2).ok_or(Error::Limit)?,
        ..Usage::default()
    })
}

/// Read and verify exact original bytes, returning complete logical output, owned encoded result and a no-change effect.
/// No partial read or progress value becomes a successful Completion; the caller still owns decision/charging.
/// # Errors
/// Limit rejects capacity before reading; Storage rejects missing, short or hash-mismatched backing bytes.
pub fn complete(pin: &Pinned, limits: Limits) -> Result<Completion, Error> {
    tracing::trace!("Constructing a complete verified Storage range result");
    reservation(pin, limits)?;
    let data = pin.bytes().verified(
        pin.lease().content_hash,
        limits.values.bytes,
        limits.object_bytes,
    )?;
    let output = Value::Object(vec![
        ("operation".into(), Value::String("retrieve".into())),
        ("leaseId".into(), Value::Bytes(pin.lease().id.to_vec())),
        ("offset".into(), Value::Uint64(pin.offset())),
        ("data".into(), Value::Bytes(data)),
        (
            "contentHash".into(),
            Value::Bytes(pin.lease().content_hash.to_vec()),
        ),
        ("price".into(), Value::Uint64(pin.price())),
    ]);
    super::operation::output(
        &output,
        super::operation::Operation::Retrieve,
        limits.values,
    )?;
    let encoded = services::codec::encode(&output, limits.values, limits.result_bytes)?;
    let effect = Effect::NoChange.encode(image::Limits {
        bytes: limits.result_bytes,
        leases: 1,
        operations: 1,
        values: limits.values,
    })?;
    Ok(Completion {
        output,
        result: Blob::new(encoded, limits.result_bytes)?,
        price: pin.price(),
        effect: Blob::new(effect, limits.result_bytes)?,
    })
}

/// Retain the original accepted read result once; identical local repetition recovers retained bytes without another read.
/// Call through trusted owner admission after reserving returned Usage; only Services decides commitment.
/// # Errors
/// Conflict rejects changed original scope; Unknown preserves uncertain durable staging and must retain holds.
/// Complete read/capacity/persistence failures preserve their original typed meaning.
pub fn stage(journal: &Journal, pin: &Pinned, limits: Limits) -> Result<Completion, Error> {
    tracing::trace!("Retaining original accepted Storage retrieval result without replaying work");
    let binding = binding(pin, journal.image_limits().bytes)?;
    if let Some(result) =
        journal.original_result(pin.key(), binding, limits.values, limits.result_bytes)?
    {
        return Ok(result);
    }
    let result = complete(pin, limits)?;
    journal.stage(pin.key(), binding, result)
}

// Length-frame original identities, scope, exact accepted terms and range fields before local hashing.
fn binding(pin: &Pinned, maximum: usize) -> Result<[u8; 32], Error> {
    tracing::trace!("Binding retained Storage retrieval to its original accepted scope");
    let key = pin.key();
    let stream = key.stream.to_be_bytes();
    let offset = pin.offset().to_be_bytes();
    let length = pin.bytes().len().to_be_bytes();
    let price = pin.price().to_be_bytes();
    let fields: [&[u8]; 11] = [
        b"TalkSphere Storage retrieval v1",
        &key.requester.0,
        &key.connection,
        &stream,
        &pin.lease().provider.0,
        &pin.lease().id,
        &pin.lease().content_hash,
        &offset,
        &length,
        &price,
        pin.accepted_terms().bytes(),
    ];
    let size = fields.iter().try_fold(0usize, |n, b| {
        n.checked_add(8)
            .and_then(|n| n.checked_add(b.len()))
            .ok_or(Error::Limit)
    })?;
    if size > maximum {
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

#[cfg(test)]
#[path = "tests/retrieve_result.rs"]
mod tests;
