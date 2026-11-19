//! Storage paid check rules validate original evidence separately from private expected bytes and requester timing.
use super::{
    Client,
    input::{field, number},
};
use crate::modules::{
    board::offering::{Direction, Offering},
    services::{
        Error,
        checks::{Invocation, Mode, Rules, Timing},
        handler::Check,
        value::Value,
    },
    storage::{
        check_contract,
        contract::State,
        operation, renewal,
        requester::byte_check::{self, Expected},
        terms::Terms,
    },
};
impl Rules for Client {
    // Every Storage challenge selects current provided terms and a separate positive check price.
    fn mode(&self, offering: &Offering, challenge: &Value) -> Result<Mode, Error> {
        tracing::trace!("Selecting separately paid Storage byte-check mode");
        self.shape(challenge)?;
        provided(offering)?;
        Ok(Mode::Paid)
    }
    // Convert exact offered seconds to the existing bounded local millisecond contract without wrapping.
    fn response_milliseconds(&self, offering: &Offering) -> Result<u64, Error> {
        tracing::trace!("Reading current Storage byte-check response interval");
        provided(offering)?
            .check_response_seconds
            .checked_mul(1000)
            .ok_or(Error::Limit)
    }
    // Validate only immutable public challenge fields; expected bytes and correctness conclusions are never accepted here.
    fn shape(&self, challenge: &Value) -> Result<(), Error> {
        tracing::trace!(
            "Validating public Storage challenge shape independently of private expectations"
        );
        crate::modules::services::validation::validate(
            challenge,
            &self.definition.document.value()["check"]["input"],
            self.policy.values,
            false,
        )?;
        operation::challenge(challenge, self.policy.values)
    }
    // Bind ownership and range to the original attempt; provider execution independently rechecks actual active retention at acceptance.
    fn challenge(&self, invocation: &Invocation, challenge: &Value) -> Result<(), Error> {
        tracing::trace!(
            "Authorizing Storage challenge against original provider-scoped lease expectations"
        );
        self.shape(challenge)?;
        check_contract::binding(invocation, invocation.provider)?;
        if !self.definition.same_contract(&invocation.definition) {
            return Err(Error::Invalid);
        }
        let lease = self.original(
            invocation.key,
            invocation.provider,
            invocation.requester,
            renewal::id(challenge)?,
        )?;
        if lease.state != State::Active {
            return Err(Error::Unavailable);
        }
        if number(challenge, "offset")? >= lease.size_bytes {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    // The provider evidence schema deliberately omits the requester-owned logical outcome and private expected byte.
    fn evidence_schema(&self) -> Result<serde_json::Value, Error> {
        tracing::trace!("Describing exact Storage provider byte evidence fields");
        Ok(serde_json::json!({"type":"object","fields":{
            "leaseId":{"type":"bytes","description":"Original provider-issued lease identity."},
            "offset":{"type":"uint64","description":"Original zero-based challenged byte position."},
            "value":{"type":"bytes","description":"Exactly one provider evidence byte."}
        }}))
    }
    // Exact original challenge association and complete one-byte shape are required before evidence can authorize settlement.
    fn evidence(&self, challenge: &Value, evidence: &Value) -> Result<(), Error> {
        tracing::trace!(
            "Checking complete Storage byte evidence association without trusting correctness claims"
        );
        self.shape(challenge)?;
        operation::evidence(evidence, self.policy.values)?;
        if field(challenge, "leaseId")? != field(evidence, "leaseId")?
            || number(challenge, "offset")? != number(evidence, "offset")?
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    // Services supplies original trusted timing; missing/ambiguous observations and private expectations never become Pass.
    fn outcome(
        &self,
        challenge: &Value,
        evidence: Option<&Value>,
        expectations: Option<&Value>,
        timing: Timing,
    ) -> Check {
        tracing::trace!(
            "Computing requester-owned Storage byte-check conclusion from private expectations"
        );
        if !matches!(timing, Timing::Within(_)) {
            return Check::Inconclusive;
        }
        let Some(expectations) = expectations else {
            return Check::Inconclusive;
        };
        let Some(evidence) = evidence else {
            return Check::Inconclusive;
        };
        let Some(expected) = private_expected(expectations, self.policy.values) else {
            return Check::Inconclusive;
        };
        if self.evidence(challenge, evidence).is_err() {
            return Check::Inconclusive;
        }
        byte_check::compare(challenge, evidence, &expected, self.policy.values)
    }
}
// Only current provided offerings support later checks; unknown pricing models or unsafe maxima remain explicit errors.
fn provided(offering: &Offering) -> Result<Terms, Error> {
    tracing::trace!("Checking current provided Storage byte-check terms");
    let terms = Terms::parse(offering)?;
    if offering.direction != Direction::Provided {
        return Err(Error::Denied);
    }
    Ok(terms)
}
// Parse strictly requester-local expectation fields; ordinary provider evidence cannot masquerade as this shape.
fn private_expected(
    value: &Value,
    budget: crate::modules::services::value::Budget,
) -> Option<Expected> {
    tracing::trace!("Reading privately owned Storage byte-check expectations");
    value.bounded(budget).ok()?;
    if field(value, "active").ok() != Some(&Value::Bool(true)) {
        return None;
    }
    let Value::Bytes(bytes) = field(value, "expectedByte").ok()? else {
        return None;
    };
    let [byte] = bytes.as_slice() else {
        return None;
    };
    Some(Expected {
        lease_id: renewal::id(value).ok()?,
        offset: number(value, "offset").ok()?,
        byte: *byte,
    })
}
