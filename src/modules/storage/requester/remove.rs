//! Requester removal validation treats success as an ended availability obligation, never physical erasure or a refund.
use crate::modules::{
    board::offering::Direction,
    services::{
        Error,
        handler::Invocation,
        pricing::Price,
        validation,
        value::{Budget, Value},
    },
    storage::{
        contract::{Lease, State},
        operation::{self, Operation},
        renewal,
        terms::Terms,
    },
};

/// Validate complete acknowledged removal output under the original accepted current offering and return terminal lease metadata.
/// The caller supplies independently retained lease data and admits successful Services completion first.
/// Only logical state changes; bytes and prior charges are not erased or refunded by this function.
/// # Errors
/// Denied rejects inconsistent roles; Invalid malformed output, IDs, factors or prices;
/// Unsupported rejects another contract; Limit preserves value and offering arithmetic bounds.
pub fn validate(
    invocation: &Invocation,
    input: &Value,
    output: &Value,
    lease: &Lease,
    budget: Budget,
) -> Result<Lease, Error> {
    tracing::trace!("Validating Storage removal against independently retained requester metadata");
    super::super::binding::check(invocation)?;
    if invocation.offering.direction != Direction::Provided
        || invocation.requester != lease.requester
        || invocation.provider != lease.provider
    {
        return Err(Error::Denied);
    }
    let definition = invocation.definition.document.value();
    validation::validate(input, &definition["input"], budget, false)?;
    validation::validate(output, &definition["output"], budget, false)?;
    if operation::input(input, budget)? != Operation::Remove
        || renewal::id(input)? != lease.id
        || !matches!(&invocation.factors,Value::Object(f) if f.is_empty())
    {
        return Err(Error::Invalid);
    }
    operation::output(output, Operation::Remove, budget)?;
    let Value::Object(fields) = output else {
        return Err(Error::Invalid);
    };
    let price = Terms::parse(&invocation.offering)?.remove_price;
    if invocation.pricing != Price::Fixed(price)
        || invocation.maximum != price
        || renewal::number(output, "price")? != price
        || !matches!(fields.iter().find(|(n,_)|n=="leaseId").map(|(_,v)|v),Some(Value::Bytes(id)) if id.as_slice()==lease.id)
    {
        return Err(Error::Invalid);
    }
    let mut removed = lease.clone();
    removed.state = State::Removed;
    Ok(removed)
}
#[cfg(test)]
#[path = "../tests/removal_validation.rs"]
mod tests;
