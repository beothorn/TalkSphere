//! Requester renewal output validation preserves immutable lease metadata while checking the original expected-expiry guard.
use crate::modules::{
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
        renewal,
    },
};

/// Validate complete authenticated renewal output and return updated independent requester metadata.
/// The caller admits successful Services completion and retains the original lease and accepted invocation.
/// Only expiration changes; later local inactive state does not prove the original accepted request was invalid.
/// # Errors
/// Denied rejects inconsistent original roles; Invalid malformed output, pricing or metadata;
/// Conflict stale original guard; Limit current retention/arithmetic bounds; Unsupported another contract.
pub fn validate(
    invocation: &Invocation,
    input: &Value,
    output: &Value,
    lease: &Lease,
    budget: Budget,
) -> Result<Lease, Error> {
    tracing::trace!("Validating committed Storage renewal against independent requester metadata");
    super::super::binding::check(invocation)?;
    if invocation.requester != lease.requester || invocation.provider != lease.provider {
        return Err(Error::Denied);
    }
    let definition = invocation.definition.document.value();
    validation::validate(input, &definition["input"], budget, false)?;
    validation::validate(output, &definition["output"], budget, false)?;
    operation::output(output, Operation::Renew, budget)?;
    let expected = renewal::price(
        &invocation.offering,
        input,
        &invocation.factors,
        lease,
        budget,
    )?;
    let Price::Fixed(price) = expected else {
        return Err(Error::Invalid);
    };
    let expires = lease
        .expires_at
        .checked_add(renewal::number(input, "additionalSeconds")?)
        .ok_or(Error::Limit)?;
    let Value::Object(fields) = output else {
        return Err(Error::Invalid);
    };
    if invocation.pricing != expected
        || invocation.maximum != price
        || renewal::number(output, "price")? != price
        || renewal::number(output, "previousExpiresAt")? != lease.expires_at
        || renewal::number(output, "expiresAt")? != expires
        || !matches!(fields.iter().find(|(n,_)|n=="leaseId").map(|(_,v)|v),Some(Value::Bytes(id)) if id.as_slice()==lease.id)
    {
        return Err(Error::Invalid);
    }
    let mut updated = lease.clone();
    updated.expires_at = expires;
    Ok(updated)
}
#[cfg(test)]
#[path = "../tests/renewal_validation.rs"]
mod tests;
