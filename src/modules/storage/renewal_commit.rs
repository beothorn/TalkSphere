//! Bounded original-key renewal staging through recovered owner admission and coordinator-owned participant decisions.
use super::{contract::Time, effect::Effect, recovery::Gate, renewal};
use crate::modules::services::{
    self, Error,
    handler::{Completion, Invocation, Peer},
    value::{Budget, Value},
};

/// Stage one original accepted renewal under recovered admission without applying its expiration or charging it.
/// Services must already have accepted the exact quote and reserved payment authority before calling.
/// Guarded preparation/commit through Gate serialize conflicting renew/remove effects; no timeout implies abort.
/// # Errors
/// Preserves authorization, shape, price, clock, guard, capacity and persistence errors;
/// Unknown retains resource holds and closes admission until owner/coordinator recovery.
pub fn stage(
    gate: &Gate,
    invocation: &Invocation,
    input: &Value,
    now: Time,
    reboot_elapsed: Option<u64>,
    budget: Budget,
) -> Result<Completion, Error> {
    tracing::trace!("Staging an original accepted Storage renewal under recovered owner admission");
    super::binding::check(invocation)?;
    if invocation.pricing != services::pricing::Price::Fixed(invocation.maximum) {
        return Err(Error::Invalid);
    }
    let definition = invocation.definition.document.value();
    services::validation::validate(input, &definition["input"], budget, false)?;
    if super::operation::input(input, budget)? != super::operation::Operation::Renew {
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
        if image.provider != invocation.provider {
            return Err(Error::Denied);
        }
        let peer = Peer {
            local: invocation.provider,
            remote: invocation.requester,
            key: invocation.key,
        };
        let assessed = renewal::assess(
            renewal::Context {
                peer: &peer,
                offering: &invocation.offering,
                image: &image,
                now,
                reboot_elapsed,
            },
            &services::input::Input::Complete(input.clone()),
            budget,
            invocation.maximum,
        )?;
        if invocation.pricing != services::pricing::Price::Fixed(assessed.price)
            || invocation.maximum != assessed.price
            || invocation.factors != assessed.assessment.factors
        {
            return Err(Error::Invalid);
        }
        let id = renewal::id(input)?;
        let mut lease = image
            .leases
            .iter()
            .find(|l| l.id == id)
            .ok_or(Error::Denied)?
            .clone();
        let previous = lease.expires_at;
        lease.expires_at = assessed.expires_at;
        let output = Value::Object(vec![
            ("operation".into(), Value::String("renew".into())),
            ("leaseId".into(), Value::Bytes(id.to_vec())),
            ("previousExpiresAt".into(), Value::Uint64(previous)),
            ("expiresAt".into(), Value::Uint64(lease.expires_at)),
            ("price".into(), Value::Uint64(assessed.price)),
        ]);
        super::operation::output(&output, super::operation::Operation::Renew, limits.values)?;
        super::mutation_result::retain(
            journal,
            quota,
            invocation.key,
            binding,
            output,
            assessed.price,
            Effect::Renew {
                expected_expires_at: previous,
                lease,
            },
        )
    })
}
#[cfg(test)]
#[path = "tests/renewal_commit.rs"]
mod tests;
