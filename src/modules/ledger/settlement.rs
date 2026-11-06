//! Durable participant preparation; the authorized Services coordinator owns decisions.
use super::{
    Error,
    authority::Action,
    contract::{Context, OperationKey},
    persistence::Persistence,
    reservation::{Reservation, Status},
    state::Ledger,
};
use crate::modules::identity::api::IdentityId;
use sha2::{Digest, Sha256};
/// Opaque exact-effect token; no peer payload constructor or administrative authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    pub(super) local: IdentityId,
    pub(super) key: OperationKey,
    pub(super) digest: [u8; 32],
}
impl<P: Persistence> Ledger<P> {
    /// Stage one validated final price and durable complete result/effect reference.
    /// Caller must retain the actual complete result and prove service validation before this call.
    /// # Errors
    /// Denied/foreign reservation, missing key, changed effect, invalid price/reference or storage fails.
    pub fn prepare(
        &self,
        context: &Context<'_>,
        reservation: &Reservation,
        final_price: u64,
        effect: Vec<u8>,
    ) -> Result<Prepared, Error> {
        tracing::trace!("Preparing exact durable Ledger settlement effect");
        if reservation.local != self.local {
            return Err(Error::Denied);
        }
        if effect.is_empty() || effect.len() > self.limits.state_bytes {
            return Err(Error::Invalid);
        }
        self.change(context, Action::Coordinate, |s| {
            let o = s
                .operations
                .get_mut(&reservation.key)
                .ok_or(Error::Denied)?;
            if final_price == 0
                || final_price > o.terms.maximum
                || (o.terms.fixed && final_price != o.terms.maximum)
            {
                return Err(Error::Invalid);
            }
            if let Some((price, stored)) = &o.effect {
                if *price != final_price || *stored != effect {
                    return Err(Error::Conflict);
                }
            } else {
                if o.status != Status::Reserved {
                    return Err(Error::Conflict);
                }
                o.effect = Some((final_price, effect.clone()));
                o.status = Status::Prepared;
            }
            Ok(Prepared {
                local: self.local,
                key: reservation.key,
                digest: token(reservation.key, final_price, &effect),
            })
        })
    }
    /// Apply one authorized durable coordinator commit decision to the exact prepared effect.
    /// # Errors
    /// Denied token, conflicting terminal decision, capacity or persistence failures prevent success.
    pub fn commit(&self, context: &Context<'_>, prepared: &Prepared) -> Result<(), Error> {
        tracing::trace!("Applying immutable Ledger commit decision");
        if prepared.local != self.local {
            return Err(Error::Denied);
        }
        self.change(context, Action::Coordinate, |s| {
            let mut o = s
                .operations
                .get(&prepared.key)
                .cloned()
                .ok_or(Error::Denied)?;
            let (price, effect) = o.effect.as_ref().ok_or(Error::Conflict)?;
            if token(prepared.key, *price, effect) != prepared.digest {
                return Err(Error::Denied);
            }
            if o.status == Status::Committed {
                return Ok(());
            }
            if o.status != Status::Prepared {
                return Err(Error::Conflict);
            }
            let price = *price;
            if o.mode == super::terms::Mode::ThirdParty {
                o.held = if self.local == o.terms.requester {
                    price
                        .checked_add(o.terms.fee.ok_or(Error::Invalid)?)
                        .ok_or(Error::Limit)?
                } else {
                    price
                };
            } else {
                match o.resource {
                    super::reservation::Resource::Owned(id) => {
                        let balance = s.accounts.owned.get_mut(&id).ok_or(Error::Insufficient)?;
                        *balance = balance.checked_sub(price).ok_or(Error::Insufficient)?;
                    }
                    super::reservation::Resource::Issued(id) => {
                        let balance = s.accounts.issued.get_mut(&id).ok_or(Error::Insufficient)?;
                        *balance = balance.checked_sub(price).ok_or(Error::Insufficient)?;
                    }
                    super::reservation::Resource::Issuance => {
                        s.issuance = s.issuance.checked_sub(price).ok_or(Error::Insufficient)?;
                        let balance = s.accounts.issued.entry(o.terms.provider).or_default();
                        *balance = balance.checked_add(price).ok_or(Error::Limit)?;
                    }
                    super::reservation::Resource::Internal => {
                        s.internal = s.internal.checked_sub(price).ok_or(Error::Insufficient)?;
                    }
                    super::reservation::Resource::Incoming(id) => {
                        let balance = s.accounts.owned.entry(id).or_default();
                        *balance = balance.checked_add(price).ok_or(Error::Limit)?;
                    }
                    super::reservation::Resource::IssuedRoom(_) => return Err(Error::Invalid),
                }
            }
            if o.mode != super::terms::Mode::ThirdParty {
                o.payment = super::payment::Payment::Settled;
            }
            o.status = Status::Committed;
            s.operations.insert(prepared.key, o);
            Ok(())
        })
    }
    /// Release a reserved operation after explicit failure, or apply a recorded prepared abort.
    /// Prepared abort requires the exact token; absence of a decision is never a timeout abort.
    /// # Errors
    /// Denies foreign holds/tokens, rejects committed operations and conflicting terminal decisions.
    pub fn abort(
        &self,
        context: &Context<'_>,
        reservation: &Reservation,
        prepared: Option<&Prepared>,
    ) -> Result<(), Error> {
        tracing::trace!("Applying explicit uncommitted Ledger abort decision");
        if reservation.local != self.local {
            return Err(Error::Denied);
        }
        self.change(context, Action::Coordinate, |s| {
            let o = s
                .operations
                .get_mut(&reservation.key)
                .ok_or(Error::Denied)?;
            if o.status == Status::Committed {
                return Err(Error::Conflict);
            }
            if let Some((price, effect)) = &o.effect {
                let p = prepared.ok_or(Error::Denied)?;
                if p.local != self.local
                    || p.key != reservation.key
                    || p.digest != token(reservation.key, *price, effect)
                {
                    return Err(Error::Denied);
                }
            } else if prepared.is_some() {
                return Err(Error::Denied);
            }
            o.status = Status::Aborted;
            Ok(())
        })
    }
    /// Query one exact durable operation status without creating or re-executing work.
    /// # Errors
    /// Requires Coordinator authority and a reconciled local image.
    pub fn status(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<Option<Status>, Error> {
        tracing::trace!("Querying durable Ledger participant status");
        context.check(&self.authority, self.local, Action::Coordinate, 0)?;
        let i = self.inner.lock().map_err(|_| Error::Unknown)?;
        if i.uncertain {
            return Err(Error::Unknown);
        }
        Ok(i.state.operations.get(&key).map(|o| o.status))
    }
}
// Domain-bind immutable effect tokens to their exact local attempt and final charge.
pub(super) fn token(key: OperationKey, price: u64, effect: &[u8]) -> [u8; 32] {
    tracing::trace!("Binding Ledger preparation to exact accepted effect");
    let mut h = Sha256::new();
    h.update(b"TalkSphere Ledger local effect v1");
    h.update(key.requester.0);
    h.update(key.connection);
    h.update(key.stream.to_be_bytes());
    h.update(price.to_be_bytes());
    h.update(effect);
    h.finalize().into()
}
