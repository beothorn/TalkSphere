//! Authorized separately paid single-byte evidence with bounded optional overlap and one-second admission rules.
use super::{
    bytes::{Range, Store},
    contract::Time,
    image::Image,
    lease, operation, renewal,
};
use crate::modules::services::{
    Error,
    checks::{Invocation, Stamp},
    validation,
    value::{Budget, Value},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, Weak},
};

/// Explicit optional provider frequency policy; no configurable stricter hidden interval exists.
#[derive(Clone, Copy, Default)]
pub struct Policy {
    /// Refuse another check while a prior accepted provider evidence pin for this lease remains alive.
    pub reject_overlap: bool,
    /// Enforce exactly a one-second minimum between accepted checks using trusted comparable millisecond observations.
    pub minimum_interval: bool,
}
struct Entry {
    last: Stamp,
    active: Weak<()>,
}
/// Bounded process-local admission state; losing this state permits more checks, never a stricter undisclosed limit.
pub struct Admission {
    policy: Policy,
    maximum: usize,
    entries: Mutex<BTreeMap<[u8; 32], Entry>>,
}
/// Trusted provider acceptance context, separate from requester timing used to judge evidence.
pub struct Context<'a> {
    /// Original authenticated explicitly paid accepted check invocation from Services.
    pub invocation: &'a Invocation,
    /// Authoritative owner image under recovered admission.
    pub image: &'a Image,
    /// Protected immutable provider byte owner.
    pub bytes: &'a Store,
    /// Coherent provider retention observation.
    pub now: Time,
    /// Trusted elapsed seconds across boot change, absent within a stable boot.
    pub reboot_elapsed: Option<u64>,
    /// Provider monotonic millisecond observation for the optional one-second interval.
    pub accepted: Stamp,
}
/// Timely original single-byte pin; expiration/removal cannot revoke its accepted read.
pub struct Pinned {
    id: [u8; 32],
    offset: u64,
    range: Range,
    _active: Arc<()>,
}
impl Admission {
    /// Configure explicit optional refusal policy and a positive finite lease-history entry bound.
    /// # Errors
    /// Invalid rejects zero capacity; this creates no offering, financial reservation or paid entitlement.
    pub fn new(policy: Policy, maximum_leases: usize) -> Result<Self, Error> {
        tracing::trace!("Configuring bounded Storage byte-check admission policy");
        if maximum_leases == 0 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            policy,
            maximum: maximum_leases,
            entries: Mutex::new(BTreeMap::new()),
        })
    }
    /// Authorize active ownership and offset under separately accepted current checkPrice, then pin one exact original byte.
    /// Use only after Services check consent/reservation through recovered owner admission; no expected byte is accepted.
    /// # Errors
    /// Denied collapses missing/foreign ownership; Unavailable inactive/expired/refused overlap or interval;
    /// Invalid malformed challenge/price/timing; Limit bounded capacity/overflow; backing failures preserve Storage.
    pub fn accept(
        &self,
        context: Context<'_>,
        challenge: &Value,
        budget: Budget,
    ) -> Result<Pinned, Error> {
        tracing::trace!("Accepting a separately paid authorized Storage byte challenge");
        let Context {
            invocation,
            image,
            bytes,
            now,
            reboot_elapsed,
            accepted,
        } = context;
        super::check_contract::binding(invocation, image.provider)?;
        validation::validate(
            challenge,
            &invocation.definition.document.value()["check"]["input"],
            budget,
            false,
        )?;
        operation::challenge(challenge, budget)?;
        let id = renewal::id(challenge)?;
        let offset = renewal::number(challenge, "offset")?;
        let lease = lease::authorize(
            image.leases.iter().find(|l| l.id == id),
            invocation.provider,
            invocation.requester,
            &id,
        )?;
        if !super::retention::active(image, lease, now, reboot_elapsed)? {
            return Err(Error::Unavailable);
        }
        if offset >= lease.size_bytes {
            return Err(Error::Invalid);
        }
        let mut entries = self.entries.lock().map_err(|_| Error::Unavailable)?;
        if let Some(prior) = entries.get(&id) {
            if self.policy.reject_overlap && prior.active.strong_count() > 0 {
                return Err(Error::Unavailable);
            }
            if self.policy.minimum_interval {
                if accepted.epoch != prior.last.epoch {
                    return Err(Error::Invalid);
                }
                let elapsed = accepted
                    .milliseconds
                    .checked_sub(prior.last.milliseconds)
                    .ok_or(Error::Invalid)?;
                if elapsed < 1000 {
                    return Err(Error::Unavailable);
                }
            }
        } else if entries.len() >= self.maximum {
            return Err(Error::Limit);
        }
        let range = bytes.reopen(id, lease.size_bytes)?.range(offset, 1)?;
        let active = Arc::new(());
        entries.insert(
            id,
            Entry {
                last: accepted,
                active: Arc::downgrade(&active),
            },
        );
        Ok(Pinned {
            id,
            offset,
            range,
            _active: active,
        })
    }
}
impl Pinned {
    /// Read exactly one accepted byte and return only leaseId, offset and value; private expectations/outcome/price never appear.
    /// The caller retains this pin through provider evidence production, then Services retains complete original evidence.
    /// # Errors
    /// Storage rejects a missing/short backing byte; no successful partial evidence is returned.
    pub fn evidence(&self) -> Result<Value, Error> {
        tracing::trace!(
            "Reading exact single-byte Storage evidence without provider correctness claims"
        );
        let value = self.range.read(0, 1, 1)?;
        Ok(Value::Object(vec![
            ("leaseId".into(), Value::Bytes(self.id.to_vec())),
            ("offset".into(), Value::Uint64(self.offset)),
            ("value".into(), Value::Bytes(value)),
        ]))
    }
}
#[cfg(test)]
#[path = "tests/byte_check.rs"]
mod tests;
