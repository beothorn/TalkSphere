//! Store output verification against original requester data and immutable accepted invocation terms.
use crate::modules::{
    cryptography::primitives::hash::sha256,
    services::{
        Error,
        handler::Invocation,
        pricing::Price,
        validation,
        value::{Budget, Value},
    },
    storage::{
        assessment, clock_policy,
        contract::{Lease, State},
        operation::{self, Operation},
        retention,
    },
};

/// Explicit requester-local clock comparison policy; a peer cannot choose either value.
#[derive(Clone, Copy)]
pub struct Policy {
    /// Independently observed requester UTC, in whole Unix seconds.
    pub observed_unix_seconds: u64,
    /// Maximum permitted apparent difference from the returned acceptance time, in seconds.
    pub maximum_clock_difference_seconds: u64,
}

/// Validate complete authenticated store output against original input and immutable accepted terms.
/// The caller supplies a trusted Services Invocation and performs normal stream completion/schema admission first.
/// Returns owned lease metadata for later catalog retention, not proof of physical provider durability.
/// # Errors
/// Denied rejects inconsistent original roles/issuer; Invalid rejects malformed or inconsistent content/prices/times;
/// Unsupported rejects other contracts, and Limit rejects bounds or expiration overflow.
pub fn validate(
    invocation: &Invocation,
    input: &Value,
    output: &Value,
    budget: Budget,
    policy: Policy,
) -> Result<Lease, Error> {
    tracing::trace!("Verifying Storage store result against original requester expectations");
    let offering = &invocation.offering;
    let definition = invocation.definition.document.value();
    super::super::binding::check(invocation)?;
    validation::validate(input, &definition["input"], budget, false)?;
    validation::validate(output, &definition["output"], budget, false)?;
    if operation::input(input, budget)? != Operation::Store {
        return Err(Error::Invalid);
    }
    operation::output(output, Operation::Store, budget)?;
    let metadata = assessment::requester_metadata(input, offering, budget)?;
    let expected = assessment::store_price(offering, &metadata, &invocation.factors)?;
    let Price::Fixed(price) = expected else {
        return Err(Error::Invalid);
    };
    if invocation.pricing != expected
        || invocation.maximum != price
        || number(output, "price")? != price
    {
        return Err(Error::Invalid);
    }
    let size = number(input, "sizeBytes")?;
    let content_hash = fixed(input, "contentHash")?;
    let Value::Bytes(data) = field(input, "data")? else {
        return Err(Error::Invalid);
    };
    if sha256(data, data.len()).map_err(|_| Error::Limit)? != content_hash
        || number(output, "sizeBytes")? != size
        || fixed::<32>(output, "contentHash")? != content_hash
    {
        return Err(Error::Invalid);
    }
    let accepted_at = number(output, "acceptedAt")?;
    let expires_at = number(output, "expiresAt")?;
    if expires_at != retention::expiration(accepted_at, number(input, "retentionSeconds")?)? {
        return Err(Error::Invalid);
    }
    clock_policy::validate(
        accepted_at,
        policy.observed_unix_seconds,
        policy.maximum_clock_difference_seconds,
    )?;
    Ok(Lease {
        id: fixed(output, "leaseId")?,
        requester: invocation.requester,
        provider: invocation.provider,
        accepted_terms: invocation.accepted.clone(),
        size_bytes: size,
        content_hash,
        accepted_at,
        expires_at,
        store_price: price,
        state: State::Active,
    })
}
// Borrow a required logical object field only after duplicate/type/budget validation.
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    tracing::trace!("Reading original Storage store verification field");
    let Value::Object(fields) = value else {
        return Err(Error::Invalid);
    };
    fields
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .ok_or(Error::Invalid)
}
// Preserve exact uint64 widths rather than relying on floating-point or JSON coercion.
fn number(value: &Value, name: &str) -> Result<u64, Error> {
    tracing::trace!("Checking exact Storage store verification integer");
    match field(value, name)? {
        Value::Uint64(n) => Ok(*n),
        _ => Err(Error::Invalid),
    }
}
// IDs and hashes retain their protocol-specified fixed byte widths.
fn fixed<const N: usize>(value: &Value, name: &str) -> Result<[u8; N], Error> {
    tracing::trace!("Checking fixed Storage store verification bytes");
    match field(value, name)? {
        Value::Bytes(b) => b.as_slice().try_into().map_err(|_| Error::Invalid),
        _ => Err(Error::Invalid),
    }
}
#[cfg(test)]
#[path = "../tests/store_result.rs"]
mod tests;
