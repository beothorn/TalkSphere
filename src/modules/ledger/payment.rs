//! Third-party confirmation is distinct from local service completion and forwarded requester data.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Context, OperationKey},
    persistence::Persistence,
    reservation::Status,
    state::Ledger,
    terms::Mode,
    transfer_state::TransferResult,
};
use crate::modules::identity::api::IdentityId;
use std::sync::Arc;
/// Durable payment resolution, independent of a committed service result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payment {
    /// Complete service work awaits original issuer confirmation.
    Pending,
    /// Issuer delivery/commitment outcome is unknown.
    Unknown,
    /// Issuer explicitly refused; no confirmed payment is claimed.
    Unsettled,
    /// Exact original issuer evidence applied once locally.
    Settled,
}
/// Trusted original issuer evidence; only composition granted Evidence authority constructs this.
pub struct Receipt {
    authority: Arc<Authority>,
    issuer: IdentityId,
    result: TransferResult,
}
impl Receipt {
    /// Bind a complete response from an independently authenticated original issuer connection.
    /// The adapter must authenticate issuer from Transport, never a forwarded JSON field.
    /// # Errors
    /// Evidence authority, issuer mismatch and malformed gross/fee reject before payment effects.
    pub fn authenticated(
        context: &Context<'_>,
        authority: Arc<Authority>,
        local: IdentityId,
        issuer: IdentityId,
        result: TransferResult,
    ) -> Result<Self, Error> {
        tracing::trace!("Binding trusted original issuer payment evidence");
        context.check(&authority, local, Action::Evidence, 121)?;
        if result.order.issuer != issuer {
            return Err(Error::Denied);
        }
        result.order.encode()?;
        result.received()?;
        Ok(Self {
            authority,
            issuer,
            result,
        })
    }
}
impl<P: Persistence> Ledger<P> {
    /// Confirm exactly one third-party invocation from its original issuer's complete result.
    /// # Errors
    /// Foreign evidence, wrong tuple/roles/gross/fee/net or unresolved service work cannot settle.
    pub fn confirm_payment(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        receipt: &Receipt,
    ) -> Result<(), Error> {
        tracing::trace!("Confirming exact issuer-mediated payment without re-executing work");
        if !Arc::ptr_eq(&self.authority, &receipt.authority) {
            return Err(Error::Denied);
        }
        self.change(context, Action::Coordinate, |s| {
            let mut op = s.operations.get(&key).cloned().ok_or(Error::Denied)?;
            if op.mode != Mode::ThirdParty || op.status != Status::Committed {
                return Err(Error::Conflict);
            }
            let price = op.effect.as_ref().ok_or(Error::Invalid)?.0;
            let t = &op.terms;
            let r = &receipt.result;
            if receipt.issuer != t.issuer
                || r.order.owner != t.requester
                || r.order.new_owner != t.provider
                || Some(r.order.transfer_id) != t.transfer
                || Some(r.fee) != t.fee
                || r.received()? != price
                || r.order.amount != price.checked_add(r.fee).ok_or(Error::Limit)?
            {
                return Err(Error::Denied);
            }
            if op.confirmed {
                return Ok(());
            }
            if self.local == t.requester {
                let balance = s
                    .accounts
                    .owned
                    .get_mut(&t.issuer)
                    .ok_or(Error::Insufficient)?;
                *balance = balance
                    .checked_sub(r.order.amount)
                    .ok_or(Error::Insufficient)?;
            } else {
                let balance = s.accounts.owned.entry(t.issuer).or_default();
                *balance = balance.checked_add(price).ok_or(Error::Limit)?;
            }
            op.confirmed = true;
            op.payment = Payment::Settled;
            s.operations.insert(key, op);
            Ok(())
        })
    }
    /// Record issuer refusal or loss after work while conservatively retaining unresolved holds.
    /// # Errors
    /// Missing/non-third-party/uncommitted/already settled operations reject.
    pub fn payment_problem(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        unknown: bool,
    ) -> Result<(), Error> {
        tracing::trace!("Recording unsettled or unknown issuer-mediated payment");
        self.change(context, Action::Coordinate, |s| {
            let op = s.operations.get_mut(&key).ok_or(Error::Denied)?;
            if op.mode != Mode::ThirdParty || op.status != Status::Committed || op.confirmed {
                return Err(Error::Conflict);
            }
            op.payment = if unknown {
                Payment::Unknown
            } else {
                Payment::Unsettled
            };
            Ok(())
        })
    }
    /// Observe payment state separately from the participant/service commitment state.
    /// # Errors
    /// Missing authority or unreconciled Ledger state fails without retrying work.
    pub fn payment(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<Option<Payment>, Error> {
        tracing::trace!("Reading independent payment resolution state");
        context.check(&self.authority, self.local, Action::Coordinate, 0)?;
        let i = self.inner.lock().map_err(|_| Error::Unknown)?;
        if i.uncertain {
            return Err(Error::Unknown);
        }
        Ok(i.state.operations.get(&key).map(|o| o.payment))
    }
}
/// Explicit composition request; returning it never repeats service work or initiates network I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentRequest {
    /// Exact unsigned order to submit or independently query at its issuer.
    pub order: super::transfer::Order,
    /// Exact fee bound already accepted for this invocation.
    pub fee: u64,
    /// True for local requester transfer submission; false for provider issuer query.
    pub submit: bool,
}
impl<P: Persistence> Ledger<P> {
    /// Describe one pending settlement action for trusted Peer Protocol composition.
    /// # Errors
    /// Requires Coordinator, committed third-party work and unresolved payment.
    pub fn payment_request(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<PaymentRequest, Error> {
        tracing::trace!("Describing explicit third-party transfer or status query");
        context.check(&self.authority, self.local, Action::Coordinate, 0)?;
        let i = self.inner.lock().map_err(|_| Error::Unknown)?;
        if i.uncertain {
            return Err(Error::Unknown);
        }
        let o = i.state.operations.get(&key).ok_or(Error::Denied)?;
        if o.mode != Mode::ThirdParty || o.status != Status::Committed || o.confirmed {
            return Err(Error::Conflict);
        }
        let fee = o.terms.fee.ok_or(Error::Invalid)?;
        let order = super::transfer::Order {
            issuer: o.terms.issuer,
            owner: o.terms.requester,
            new_owner: o.terms.provider,
            transfer_id: o.terms.transfer.ok_or(Error::Invalid)?,
            amount: o
                .effect
                .as_ref()
                .ok_or(Error::Invalid)?
                .0
                .checked_add(fee)
                .ok_or(Error::Limit)?,
        };
        order.encode()?;
        Ok(PaymentRequest {
            order,
            fee,
            submit: self.local == o.terms.requester,
        })
    }
}
