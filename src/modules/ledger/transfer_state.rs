//! Issuer-controlled atomic transfer accounting and permanent replay records.
use super::{
    Error,
    authority::Action,
    contract::Context,
    fees::Accepted,
    persistence::Persistence,
    reservation::{Resource, available},
    state::{Ledger, State},
    transfer::Order,
};
use crate::modules::identity::api::IdentityId;
use std::{collections::BTreeMap, sync::Arc};
/// Original durable transfer result; issuer provenance comes from authenticated context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferResult {
    /// Original exact owner, recipient, transfer ID and gross amount.
    pub order: Order,
    /// Exact accepted fee; net receipt is gross minus fee.
    pub fee: u64,
}
impl TransferResult {
    /// Return checked positive net amount from the retained canonical result.
    /// # Errors
    /// Invalid fee/gross relation rejects malformed evidence.
    pub fn received(&self) -> Result<u64, Error> {
        tracing::trace!("Computing exact net transferred credits");
        if self.fee >= self.order.amount {
            return Err(Error::Invalid);
        }
        Ok(self.order.amount - self.fee)
    }
}
impl<P: Persistence> Ledger<P> {
    /// Commit an authenticated same-stream fee acceptance atomically, or return its original replay result.
    /// # Errors
    /// Foreign acceptance, conflicting order, insufficient available source/headroom or storage fails.
    pub fn commit_transfer(
        &self,
        context: &Context<'_>,
        accepted: &Accepted,
    ) -> Result<TransferResult, Error> {
        tracing::trace!("Committing issuer-controlled transfer exactly once");
        if !Arc::ptr_eq(&self.authority, &accepted.authority) || accepted.order.issuer != self.local
        {
            return Err(Error::Denied);
        }
        self.change(context, Action::Control, |s| {
            let order = &accepted.order;
            let key = (order.owner, order.transfer_id);
            if let Some(prior) = s.transfers.get(&key) {
                if prior.order != *order {
                    return Err(Error::Conflict);
                }
                return Ok(prior.clone());
            }
            if s.transfers.len() >= self.limits.transfers {
                return Err(Error::Limit);
            }
            let result = TransferResult {
                order: order.clone(),
                fee: accepted.fee,
            };
            let net = result.received()?;
            if available(s, Resource::Issued(order.owner))? < order.amount {
                return Err(Error::Insufficient);
            }
            if available(s, Resource::IssuedRoom(order.new_owner))? < net {
                return Err(Error::Limit);
            }
            let source = s
                .accounts
                .issued
                .get_mut(&order.owner)
                .ok_or(Error::Insufficient)?;
            *source = source
                .checked_sub(order.amount)
                .ok_or(Error::Insufficient)?;
            let destination = s.accounts.issued.entry(order.new_owner).or_default();
            *destination = destination.checked_add(net).ok_or(Error::Limit)?;
            s.transfers.insert(key, result.clone());
            Ok(result)
        })
    }
    /// Retrieve an original result only for its original or new authenticated owner.
    /// # Errors
    /// Unrelated callers receive Denied without protected-result disclosure.
    pub fn transfer_result(
        &self,
        context: &Context<'_>,
        authenticated: IdentityId,
        owner: IdentityId,
        transfer_id: [u8; 16],
    ) -> Result<Option<TransferResult>, Error> {
        tracing::trace!("Querying owner-protected transfer result");
        context.check(&self.authority, self.local, Action::Control, 0)?;
        let i = self.inner.lock().map_err(|_| Error::Unknown)?;
        if i.uncertain {
            return Err(Error::Unknown);
        }
        match i.state.transfers.get(&(owner, transfer_id)) {
            Some(result) if authenticated == owner || authenticated == result.order.new_owner => {
                Ok(Some(result.clone()))
            }
            None if authenticated == owner => Ok(None),
            _ => Err(Error::Denied),
        }
    }
    /// Report only this issuer's recognized balance for the trusted adapter's authenticated caller.
    /// # Errors
    /// Control authority, self-account or uncertain state rejection preserves accounting.
    pub fn balance(&self, context: &Context<'_>, authenticated: IdentityId) -> Result<u64, Error> {
        tracing::trace!("Reporting authenticated caller-only issuer balance");
        context.check(&self.authority, self.local, Action::Control, 0)?;
        let i = self.inner.lock().map_err(|_| Error::Unknown)?;
        if i.uncertain {
            return Err(Error::Unknown);
        }
        i.state
            .accounts
            .remote_balance(context, &self.authority, self.local, authenticated)
    }
    /// Recognize accepted signed retries before quoting, so later policy changes cannot recharge them.
    /// # Errors
    /// Invalid authenticated proof or conflicting key reuse fails; absence is not an accepted transfer.
    pub fn existing_transfer(
        &self,
        context: &Context<'_>,
        authenticated: IdentityId,
        order: &Order,
        key: &[u8],
        signature: &[u8],
    ) -> Result<Option<TransferResult>, Error> {
        tracing::trace!("Recognizing original accepted transfer before requoting");
        context.check(&self.authority, self.local, Action::Control, 121)?;
        super::transfer_crypto::verify(authenticated, self.local, order, key, signature)?;
        let i = self.inner.lock().map_err(|_| Error::Unknown)?;
        if i.uncertain {
            return Err(Error::Unknown);
        }
        if let Some(prior) = i.state.transfers.get(&(order.owner, order.transfer_id)) {
            if prior.order != *order {
                return Err(Error::Conflict);
            }
            return Ok(Some(prior.clone()));
        }
        Ok(None)
    }
}
// Persist canonical order bytes and original fee; never compact lifetime replay identities.
pub(super) fn encode(s: &State) -> Result<serde_json::Value, Error> {
    tracing::trace!("Encoding lifetime transfer replay records");
    Ok(serde_json::Value::Array(
        s.transfers
            .values()
            .map(|r| Ok(serde_json::json!([r.order.encode()?.to_vec(), r.fee])))
            .collect::<Result<Vec<_>, Error>>()?,
    ))
}
// Revalidate every accepted replay row, including canonical layout, issuer and unique replay key.
pub(super) fn decode(
    v: &serde_json::Value,
    local: IdentityId,
    maximum: usize,
) -> Result<BTreeMap<(IdentityId, [u8; 16]), TransferResult>, Error> {
    tracing::trace!("Validating recovered transfer replay records");
    let rows = v.as_array().ok_or(Error::Invalid)?;
    if rows.len() > maximum {
        return Err(Error::Limit);
    }
    let mut out = BTreeMap::new();
    for row in rows {
        let row = row
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or(Error::Invalid)?;
        let order = Order::decode(&super::reservation::byte_vec(&row[0])?)?;
        if order.issuer != local {
            return Err(Error::Denied);
        }
        let result = TransferResult {
            order,
            fee: super::state::number(&row[1])?,
        };
        result.received()?;
        if out
            .insert((result.order.owner, result.order.transfer_id), result)
            .is_some()
        {
            return Err(Error::Invalid);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "tests/transfer_state.rs"]
mod tests;
