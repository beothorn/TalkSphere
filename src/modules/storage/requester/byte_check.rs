//! Requester-owned byte comparisons consume original trusted transport observations without altering payment or trust.
use crate::modules::{
    identity::api::IdentityId,
    services::{
        checks::{Invocation, Stamp},
        handler::Check,
        value::{Budget, Value},
    },
    storage::{check_contract, operation, renewal},
};

/// Private original byte expectation, retained by the requester and never included in a challenge.
#[derive(Clone, Copy)]
pub struct Expected {
    /// Exact provider-issued original lease identity.
    pub lease_id: [u8; 32],
    /// Zero-based byte position matching the independently constructed original challenge.
    pub offset: u64,
    /// Byte from the requester's original stored data, including encryption when ciphertext was uploaded.
    pub byte: u8,
}
/// Requester-local knowledge about the lease when the original challenge was accepted.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    /// Trusted lease metadata and clock policy establish activity without an expiration-boundary ambiguity.
    Active,
    /// The original request concerned an inactive lease and cannot establish an active-storage failure.
    Inactive,
    /// Clock/expiration uncertainty prevents a reliable statement about activity.
    Ambiguous,
}
/// Original local transport observations, not peer timestamps or later worker execution times.
#[derive(Clone, Copy, Default)]
pub struct Observations {
    /// Immediately after complete original challenge transmission.
    pub sent: Option<Stamp>,
    /// Original complete evidence or authenticated unavailability-report arrival.
    pub received: Option<Stamp>,
    /// Original normal response END observation, establishing complete transfer.
    pub ended: Option<Stamp>,
}
/// Authenticated original response supplied by the peer adapter; generic refusal/timeout is not proof of byte loss.
pub enum Reply<'a> {
    /// Complete evidence on the original authenticated association, without a provider-computed outcome.
    Evidence {
        /// Authenticated remote identity from Transport, never a payload identity field.
        peer: IdentityId,
        /// Complete bounded provider evidence value.
        value: &'a Value,
    },
    /// Explicit original provider report that the independently authorized active lease or requested byte is unavailable.
    Unavailable {
        /// Authenticated original provider, not an unauthenticated error or generic internal/capacity failure.
        peer: IdentityId,
    },
    /// Missing/abnormal/unauthenticated response, generic refusal, communication failure or timeout.
    Missing,
}

/// Calculate pass/fail/inconclusive from original accepted terms, private expectation and trusted original observations.
/// No current clock is sampled, expected bytes are never transmitted, and no payment, refund or trust state changes.
/// Malformed/foreign/incomplete/late responses, absent expectations and activity/timing ambiguity are inconclusive.
/// A complete timely wrong byte or explicit authorized-active unavailability report is fail.
pub fn verify(
    invocation: &Invocation,
    challenge: &Value,
    reply: Reply<'_>,
    expected: Option<&Expected>,
    activity: Activity,
    observations: Observations,
    budget: Budget,
) -> Check {
    tracing::trace!(
        "Comparing Storage byte evidence with private original data and trusted transport timing"
    );
    let Some(expected) = expected else {
        return Check::Inconclusive;
    };
    if activity != Activity::Active
        || check_contract::binding(invocation, invocation.provider).is_err()
        || operation::challenge(challenge, budget).is_err()
        || renewal::id(challenge).ok() != Some(expected.lease_id)
        || renewal::number(challenge, "offset").ok() != Some(expected.offset)
        || !timely(observations, invocation.response_milliseconds)
    {
        return Check::Inconclusive;
    }
    match reply {
        Reply::Missing => Check::Inconclusive,
        Reply::Unavailable { peer } => {
            if peer == invocation.provider {
                Check::Fail
            } else {
                Check::Inconclusive
            }
        }
        Reply::Evidence { peer, value } => {
            if peer != invocation.provider
                || operation::evidence(value, budget).is_err()
                || renewal::id(value).ok() != Some(expected.lease_id)
                || renewal::number(value, "offset").ok() != Some(expected.offset)
            {
                return Check::Inconclusive;
            }
            let Value::Object(fields) = value else {
                return Check::Inconclusive;
            };
            match fields.iter().find(|(n, _)| n == "value").map(|(_, v)| v) {
                Some(Value::Bytes(bytes)) if bytes.as_slice() == [expected.byte] => Check::Pass,
                Some(Value::Bytes(_)) => Check::Fail,
                _ => Check::Inconclusive,
            }
        }
    }
}
// Require one coherent original send/evidence/end sequence within the accepted inclusive response interval.
fn timely(observations: Observations, maximum_milliseconds: u64) -> bool {
    tracing::trace!("Checking original complete Storage challenge response observations");
    let Observations {
        sent: Some(sent),
        received: Some(received),
        ended: Some(ended),
    } = observations
    else {
        return false;
    };
    sent.epoch == received.epoch
        && received.epoch == ended.epoch
        && received.milliseconds >= sent.milliseconds
        && ended.milliseconds >= received.milliseconds
        && ended
            .milliseconds
            .checked_sub(sent.milliseconds)
            .is_some_and(|elapsed| elapsed <= maximum_milliseconds)
}
#[cfg(test)]
#[path = "../tests/byte_verification.rs"]
mod tests;
