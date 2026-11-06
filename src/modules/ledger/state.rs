//! Serialized local state owner; durable commits precede visible accounting changes.
use super::{
    Error,
    accounting::{Accounts, Trust, View},
    authority::{Action, Authority},
    contract::{Context, Limits},
    persistence::{Persistence, Record},
};
use crate::modules::identity::api::IdentityId;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
pub(super) struct State {
    pub accounts: Accounts,
    pub issuance: u64,
    pub internal: u64,
    pub operations: BTreeMap<super::contract::OperationKey, super::reservation::Operation>,
}
pub(super) struct Inner<P> {
    pub state: State,
    pub persistence: P,
    pub uncertain: bool,
}
/// One coordinated local account writer. Independent stores must never share one identity's budget.
pub struct Ledger<P: Persistence> {
    pub(super) local: IdentityId,
    pub(super) authority: Arc<Authority>,
    pub(super) limits: Limits,
    pub(super) inner: Mutex<Inner<P>>,
}
impl<P: Persistence> Ledger<P> {
    /// Explicitly initialize an absent store with zero balances and finite operator budgets.
    /// # Errors
    /// Denied authority, existing state, invalid limits or persistence failure prevents success.
    pub fn initialize(
        local: IdentityId,
        authority: Arc<Authority>,
        mut persistence: P,
        limits: Limits,
        context: &Context<'_>,
        issuance: u64,
        internal: u64,
    ) -> Result<Self, Error> {
        tracing::trace!("Initializing explicitly authorized Ledger budgets");
        limits.validate()?;
        context.check(&authority, local, Action::Administer, 0)?;
        if persistence.load()?.is_some() {
            return Err(Error::Conflict);
        }
        let state = State {
            issuance,
            internal,
            ..State::default()
        };
        let record = state.record(local, limits)?;
        context.check(&authority, local, Action::Administer, record.payload.len())?;
        persistence.commit(&record)?;
        Ok(Self {
            local,
            authority,
            limits,
            inner: Mutex::new(Inner {
                state,
                persistence,
                uncertain: false,
            }),
        })
    }
    /// Open existing complete state; never initialize or silently repair corrupt/missing data.
    /// # Errors
    /// Invalid binding/schema, authority, unavailable state and storage failures are explicit.
    pub fn open(
        local: IdentityId,
        authority: Arc<Authority>,
        mut persistence: P,
        limits: Limits,
        context: &Context<'_>,
    ) -> Result<Self, Error> {
        tracing::trace!("Opening existing Ledger state");
        limits.validate()?;
        context.check(&authority, local, Action::Read, 0)?;
        let record = persistence.load()?.ok_or(Error::Storage)?;
        context.check(&authority, local, Action::Read, record.payload.len())?;
        let state = State::load(&record, local, limits)?;
        Ok(Self {
            local,
            authority,
            limits,
            inner: Mutex::new(Inner {
                state,
                persistence,
                uncertain: false,
            }),
        })
    }
    /// Obtain copied issuer-separated state without reconciling a peer's claim.
    /// # Errors
    /// Unknown gates reads after uncertain commit; authority and self-account checks apply.
    pub fn view(&self, context: &Context<'_>, peer: IdentityId) -> Result<View, Error> {
        tracing::trace!("Reading durable Ledger view");
        context.check(&self.authority, self.local, Action::Read, 0)?;
        let inner = self.inner.lock().map_err(|_| Error::Unknown)?;
        if inner.uncertain {
            return Err(Error::Unknown);
        }
        inner
            .state
            .accounts
            .view(context, &self.authority, self.local, peer)
    }
    /// Set local exact trust under administration; authentication never calls this implicitly.
    /// # Errors
    /// Denied, Invalid self target, capacity or persistence errors preserve prior state.
    pub fn set_trust(
        &self,
        context: &Context<'_>,
        peer: IdentityId,
        trust: Trust,
    ) -> Result<(), Error> {
        tracing::trace!("Updating explicit local trust policy");
        self.peer(peer)?;
        self.change(context, Action::Administer, |s| {
            s.accounts.trust.insert(peer, trust);
            Ok(())
        })
    }
    /// Grant finite local issuance to an external owner, consuming configured issuance capacity.
    /// # Errors
    /// Rejects zero/self, exhausted budget or destination overflow before mutation.
    pub fn grant(&self, context: &Context<'_>, peer: IdentityId, amount: u64) -> Result<(), Error> {
        tracing::trace!("Granting finite locally issued credits");
        self.peer(peer)?;
        if amount == 0 {
            return Err(Error::Invalid);
        }
        self.change(context, Action::Administer, |s| {
            let available = s.issuance.checked_sub(amount).ok_or(Error::Insufficient)?;
            let next = s
                .accounts
                .issued
                .get(&peer)
                .copied()
                .unwrap_or(0)
                .checked_add(amount)
                .ok_or(Error::Limit)?;
            s.issuance = available;
            s.accounts.issued.insert(peer, next);
            Ok(())
        })
    }
    /// Explicitly install a locally reconciled owned balance; remote queries cannot invoke administration.
    /// # Errors
    /// Denied authority, self target or storage failure has no successful accounting effect.
    pub fn set_owned(
        &self,
        context: &Context<'_>,
        issuer: IdentityId,
        amount: u64,
    ) -> Result<(), Error> {
        tracing::trace!("Applying authorized owned-balance reconciliation");
        self.peer(issuer)?;
        self.change(context, Action::Administer, |s| {
            let held = s.accounts.owned.get(&issuer).copied().unwrap_or(0)
                - super::reservation::available(s, super::reservation::Resource::Owned(issuer))?;
            if amount < held {
                return Err(Error::Insufficient);
            }
            s.accounts.owned.insert(issuer, amount);
            Ok(())
        })
    }
    /// Revoke recognized local credits without restoring consumed lifetime issuance authority.
    /// # Errors
    /// Rejects self/zero and insufficient recognized balance; future holds constrain available amount.
    pub fn revoke(
        &self,
        context: &Context<'_>,
        peer: IdentityId,
        amount: u64,
    ) -> Result<(), Error> {
        tracing::trace!("Revoking explicit recognized credits");
        self.peer(peer)?;
        if amount == 0 {
            return Err(Error::Invalid);
        }
        self.change(context, Action::Administer, |s| {
            if super::reservation::available(s, super::reservation::Resource::Issued(peer))?
                < amount
            {
                return Err(Error::Insufficient);
            }
            let next = s
                .accounts
                .issued
                .get(&peer)
                .copied()
                .unwrap_or(0)
                .checked_sub(amount)
                .ok_or(Error::Insufficient)?;
            s.accounts.issued.insert(peer, next);
            Ok(())
        })
    }
    /// Return remaining finite issuance/internal budgets as a privileged observation.
    /// # Errors
    /// Denied or uncertain state prevents disclosure.
    pub fn budgets(&self, context: &Context<'_>) -> Result<(u64, u64), Error> {
        tracing::trace!("Reading remaining Ledger budgets");
        context.check(&self.authority, self.local, Action::Read, 0)?;
        let i = self.inner.lock().map_err(|_| Error::Unknown)?;
        if i.uncertain {
            return Err(Error::Unknown);
        }
        Ok((i.state.issuance, i.state.internal))
    }
    /// Reload possibly committed state without repeating any mutation.
    /// # Errors
    /// Denied authority or invalid/missing state leaves recovery gated.
    pub fn recover(&self, context: &Context<'_>) -> Result<(), Error> {
        tracing::trace!("Reconciling uncertain Ledger image");
        context.check(&self.authority, self.local, Action::Coordinate, 0)?;
        let mut i = self.inner.lock().map_err(|_| Error::Unknown)?;
        let record = i.persistence.load()?.ok_or(Error::Storage)?;
        context.check(
            &self.authority,
            self.local,
            Action::Coordinate,
            record.payload.len(),
        )?;
        let state = State::load(&record, self.local, self.limits)?;
        i.state = state;
        i.uncertain = false;
        Ok(())
    }
    // Mutate a private candidate under one writer; unknown persistence gates all dependent work.
    pub(super) fn change<T>(
        &self,
        context: &Context<'_>,
        action: Action,
        operation: impl FnOnce(&mut State) -> Result<T, Error>,
    ) -> Result<T, Error> {
        tracing::trace!("Serializing complete Ledger transaction");
        context.check(&self.authority, self.local, action, 0)?;
        let mut i = self.inner.lock().map_err(|_| Error::Unknown)?;
        if i.uncertain {
            return Err(Error::Unknown);
        }
        let mut candidate = i.state.clone();
        let result = operation(&mut candidate)?;
        let record = candidate.record(self.local, self.limits)?;
        context.check(&self.authority, self.local, action, record.payload.len())?;
        match i.persistence.commit(&record) {
            Ok(()) => i.state = candidate,
            Err(e) => {
                if e == Error::Unknown {
                    i.uncertain = true
                }
                return Err(e);
            }
        }
        Ok(result)
    }
    // Internal exact identity validation precedes any self-balance creation.
    fn peer(&self, peer: IdentityId) -> Result<(), Error> {
        tracing::trace!("Checking external Ledger account target");
        if peer == self.local {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
impl State {
    // Encode complete exact state only after cardinality, self-balance and byte bounds pass.
    pub(super) fn record(&self, local: IdentityId, limits: Limits) -> Result<Record, Error> {
        tracing::trace!("Preparing complete Ledger transaction image");
        let maps = [&self.accounts.owned, &self.accounts.issued];
        for map in maps {
            if map.len() > limits.accounts {
                return Err(Error::Limit);
            }
            if map.contains_key(&local) {
                return Err(Error::Invalid);
            }
        }
        if self.accounts.trust.len() > limits.accounts {
            return Err(Error::Limit);
        }
        let encode = |map: &BTreeMap<IdentityId, u64>| {
            map.iter()
                .map(|(k, v)| json!([k.0.to_vec(), v]))
                .collect::<Vec<_>>()
        };
        let trust = self
            .accounts
            .trust
            .iter()
            .map(|(k, v)| json!([k.0.to_vec(), v.millionths()]))
            .collect::<Vec<_>>();
        if self.operations.len() > limits.operations {
            return Err(Error::Limit);
        }
        for op in self.operations.values() {
            super::reservation::available(self, op.resource)?;
        }
        let payload=serde_json::to_vec(&json!({"version":1,"owned":encode(&self.accounts.owned),"issued":encode(&self.accounts.issued),"trust":trust,"issuance":self.issuance,"internal":self.internal,"operations":super::reservation::encode(&self.operations)})).map_err(|_|Error::Invalid)?;
        if payload.len() > limits.state_bytes {
            return Err(Error::Limit);
        }
        Ok(Record {
            identity: local,
            payload,
        })
    }
    // Decode only the private canonical schema, retaining exact unsigned integers and unique identities.
    pub(super) fn load(record: &Record, local: IdentityId, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Validating recovered Ledger accounting invariants");
        if record.identity != local {
            return Err(Error::Denied);
        }
        if record.payload.len() > limits.state_bytes {
            return Err(Error::Limit);
        }
        let v: Value = serde_json::from_slice(&record.payload).map_err(|_| Error::Invalid)?;
        if v["version"] != 1 {
            return Err(Error::Unsupported);
        }
        let mut s = State {
            issuance: number(&v["issuance"])?,
            internal: number(&v["internal"])?,
            ..State::default()
        };
        s.operations = super::reservation::decode(&v["operations"], limits.operations)?;
        s.accounts.owned = map(&v["owned"], limits.accounts)?;
        s.accounts.issued = map(&v["issued"], limits.accounts)?;
        for (id, n) in map(&v["trust"], limits.accounts)? {
            s.accounts.trust.insert(
                id,
                Trust::new(u32::try_from(n).map_err(|_| Error::Invalid)?)?,
            );
        }
        let encoded = s.record(local, limits)?;
        if encoded.payload != record.payload {
            return Err(Error::Invalid);
        }
        Ok(s)
    }
}
// Parse exact uint64 fields; no float conversion or defaulting malformed values to zero.
pub(super) fn number(v: &Value) -> Result<u64, Error> {
    tracing::trace!("Reading exact Ledger state integer");
    v.as_u64().ok_or(Error::Invalid)
}
// Reject duplicate accounts and malformed identities while reading bounded private arrays.
fn map(v: &Value, maximum: usize) -> Result<BTreeMap<IdentityId, u64>, Error> {
    tracing::trace!("Decoding bounded issuer-specific account entries");
    let entries = v.as_array().ok_or(Error::Invalid)?;
    if entries.len() > maximum {
        return Err(Error::Limit);
    }
    let mut out = BTreeMap::new();
    for pair in entries {
        let pair = pair
            .as_array()
            .filter(|p| p.len() == 2)
            .ok_or(Error::Invalid)?;
        let bytes = pair[0]
            .as_array()
            .filter(|a| a.len() == 32)
            .ok_or(Error::Invalid)?;
        let mut id = [0; 32];
        for (out, v) in id.iter_mut().zip(bytes) {
            *out = u8::try_from(number(v)?).map_err(|_| Error::Invalid)?;
        }
        if out.insert(IdentityId(id), number(&pair[1])?).is_some() {
            return Err(Error::Invalid);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
