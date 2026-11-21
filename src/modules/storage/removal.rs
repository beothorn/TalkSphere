//! Paid logical lease removal proposals preserve accepted reads and retain original participant outcomes.
use super::{
    contract::Time,
    effect::Effect,
    lease,
    operation::{self, Operation},
    recovery::Gate,
    renewal,
    terms::Terms,
};
use crate::modules::{
    board::offering::Direction,
    services::{
        Error,
        handler::{Completion, Invocation},
        pricing::Price,
        validation,
        value::{Budget, Value},
    },
};

/// Stage an authenticated original accepted removal using current provided terms and exact positive removePrice.
/// Services owns accepted payment authority and later successful output; this local port only retains a proposal.
/// # Errors
/// Denied collapses missing/foreign access; Unavailable rejects inactive/expired leases;
/// Invalid rejects malformed input, factors or price; Limit preserves bounds; Unknown gates uncertain persistence.
pub fn stage(
    gate: &Gate,
    invocation: &Invocation,
    input: &Value,
    now: Time,
    reboot_elapsed: Option<u64>,
    budget: Budget,
) -> Result<Completion, Error> {
    tracing::trace!("Staging paid logical Storage removal under original requester authority");
    super::binding::check(invocation)?;
    if invocation.offering.direction != Direction::Provided {
        return Err(Error::Denied);
    }
    validation::validate(
        input,
        &invocation.definition.document.value()["input"],
        budget,
        false,
    )?;
    if operation::input(input, budget)? != Operation::Remove
        || !matches!(&invocation.factors,Value::Object(f) if f.is_empty())
    {
        return Err(Error::Invalid);
    }
    let price = Terms::parse(&invocation.offering)?.remove_price;
    if invocation.pricing != Price::Fixed(price) || invocation.maximum != price {
        return Err(Error::Invalid);
    }
    gate.admit(|journal, quota| {
        let limits = journal.image_limits();
        let binding = super::binding::retained(invocation, input, limits)?;
        if let Some(result) =
            journal.original_result(invocation.key, binding, limits.values, limits.bytes)?
        {
            return Ok(result);
        }
        let image = journal.snapshot()?;
        if invocation.provider != image.provider {
            return Err(Error::Denied);
        }
        let id = renewal::id(input)?;
        let current = lease::authorize(
            image.leases.iter().find(|l| l.id == id),
            invocation.provider,
            invocation.requester,
            &id,
        )?;
        if !super::retention::active(&image, current, now, reboot_elapsed)? {
            return Err(Error::Unavailable);
        }
        let output = Value::Object(vec![
            ("operation".into(), Value::String("remove".into())),
            ("leaseId".into(), Value::Bytes(id.to_vec())),
            ("removed".into(), Value::Bool(true)),
            ("price".into(), Value::Uint64(price)),
        ]);
        operation::output(&output, Operation::Remove, limits.values)?;
        super::mutation_result::retain(
            journal,
            quota,
            invocation.key,
            binding,
            output,
            price,
            Effect::Remove {
                id,
                expected_expires_at: current.expires_at,
            },
        )
    })
}
#[cfg(test)]
#[path = "tests/removal.rs"]
mod tests;
