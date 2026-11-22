//! Idempotent issuance of early-release credits bound to one Services operation.
//!
//! Callers: the Messaging participant's removal settlement, as the second Ledger record of one Services decision. The removal
//! charge is the ordinary prepared/committed operation; the credits it releases are issued here under the same operation key, so
//! recovery can repeat the call after any crash and the credits are issued at most once for the ledger's lifetime.
use super::{
    Error,
    authority::Action,
    contract::{Context, OperationKey},
    persistence::Persistence,
    state::{Ledger, number},
};
use crate::modules::identity::api::IdentityId;
use serde_json::{Value, json};
use std::collections::BTreeMap;

impl<P: Persistence> Ledger<P> {
    /// Issue `amount` locally issued credits to `peer` once for `key`, as one coordinator-authorized durable record. Repeating the
    /// exact call (same peer and amount) after a crash or lost reply succeeds without issuing again.
    /// # Errors
    /// `Denied` without Coordinate authority; `Invalid` for a zero amount or the local identity as peer; `Conflict` when `key`
    /// already issued something different; `Insufficient` or `Limit` when issuance capacity or the peer's room is exhausted, with no
    /// change; `Storage` or `Unknown` as for every Ledger commit (recover before retrying after `Unknown`).
    pub fn issue_release(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        peer: IdentityId,
        amount: u64,
    ) -> Result<(), Error> {
        tracing::trace!("Issuing early-release credits once for an operation");
        if peer == self.local || amount == 0 {
            return Err(Error::Invalid);
        }
        self.change(context, Action::Coordinate, |s| {
            if let Some(done) = s.releases.get(&key) {
                return if *done == (peer, amount) {
                    Ok(())
                } else {
                    Err(Error::Conflict)
                };
            }
            s.issue(peer, amount)?;
            s.releases.insert(key, (peer, amount));
            Ok(())
        })
    }

    /// What `key` already issued, if anything, without changing state.
    /// # Errors
    /// Authority failures, or `Unknown` while a commit is uncertain.
    pub fn release_of(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<Option<(IdentityId, u64)>, Error> {
        tracing::trace!("Reading an operation's recorded release");
        context.check(&self.authority, self.local, Action::Read, 0)?;
        let inner = self.inner.lock().map_err(|_| Error::Unknown)?;
        if inner.uncertain {
            return Err(Error::Unknown);
        }
        Ok(inner.state.releases.get(&key).copied())
    }
}

// Rows of `[requester, connection, stream, peer, amount]` in key order.
pub(super) fn encode(releases: &BTreeMap<OperationKey, (IdentityId, u64)>) -> Value {
    tracing::trace!("Encoding recorded early-release issuances");
    Value::Array(
        releases
            .iter()
            .map(|(k, (peer, amount))| {
                json!([k.requester.0, k.connection, k.stream, peer.0, amount])
            })
            .collect(),
    )
}

// Decode the optional member: absent means none; rows must be exact, unique and within `maximum`.
pub(super) fn decode(
    v: &Value,
    maximum: usize,
) -> Result<BTreeMap<OperationKey, (IdentityId, u64)>, Error> {
    tracing::trace!("Decoding recorded early-release issuances");
    use super::reservation::array;
    if v.is_null() {
        return Ok(BTreeMap::new());
    }
    let rows = v.as_array().ok_or(Error::Invalid)?;
    if rows.len() > maximum {
        return Err(Error::Limit);
    }
    let mut out = BTreeMap::new();
    for row in rows {
        let a = row
            .as_array()
            .filter(|a| a.len() == 5)
            .ok_or(Error::Invalid)?;
        let key = OperationKey {
            requester: IdentityId(array(&a[0])?),
            connection: array(&a[1])?,
            stream: u32::try_from(number(&a[2])?).map_err(|_| Error::Invalid)?,
        };
        if out
            .insert(key, (IdentityId(array(&a[3])?), number(&a[4])?))
            .is_some()
        {
            return Err(Error::Invalid);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "tests/release.rs"]
mod tests;
