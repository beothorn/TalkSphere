//! Explicit retained-family replay is checked before new prices/balances and never repeats provider work.
use super::*;
use crate::modules::services::{
    handler::Peer,
    replay::{Cached, Id, Proposal, Retry},
};

impl<P: Persistence> Coordinator<P> {
    /// Find only family-approved original Messaging output for this caller and every original request field.
    /// # Errors
    /// Changed input/bindings conflict, wrong roles fail, and matching uncertain work remains Unknown instead of being re-executed.
    pub fn cached_provider(
        &self,
        context: &Context<'_>,
        peer: Peer,
        proposal: &Proposal,
    ) -> Result<Option<Cached>, Error> {
        tracing::trace!("Checking retained family replay before current terms and balances");
        if peer.local != self.config.local
            || peer.remote != peer.key.requester
            || peer.key.stream == 0
        {
            return Err(Error::Denied);
        }
        if proposal.service != "cr2se.messaging" || proposal.version != 1 {
            return Ok(None);
        }
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Observe, &state)?;
        proposal.input.bounded(Budget {
            depth: 31,
            nodes: self.config.limits.nodes,
            bytes: self.config.limits.state_bytes,
        })?;
        for op in state.records.values().filter(|o| {
            o.service == proposal.service
                && o.version == proposal.version
                && o.check.is_none()
                && o.terms.provider == self.config.local
                && o.terms.requester == peer.remote
        }) {
            if op.phase == Phase::Aborted {
                continue;
            }
            let Some(input) = &op.input else {
                continue;
            };
            let pin = state.pins.get(&op.key).ok_or(Error::Unavailable)?;
            let Some(original) = eligible(pin.requester(), input)? else {
                continue;
            };
            if eligible(pin.requester(), &proposal.input)? != Some(original) {
                continue;
            }
            let invocation = op.invocation(self.config.limits)?;
            if input != &proposal.input
                || invocation.offering.id != proposal.offering
                || invocation.publisher != proposal.publisher
                || invocation.issuer != proposal.issuer
                || op.terms.spending_bound != proposal.maximum
                || op.terms.transfer != proposal.payment.transfer
                || op.terms.fee != proposal.payment.fee
            {
                return Err(Error::Conflict);
            }
            if peer.key == op.key {
                return Err(Error::Conflict);
            }
            if op.phase != Phase::Committed {
                return Err(Error::Unknown);
            }
            if !pin
                .provider()?
                .allow_cached_retry(&invocation, peer, input)?
            {
                return Err(Error::Unsupported);
            }
            self.check(context, Action::Observe, &state)?;
            return Ok(Some(Cached {
                invocation,
                completion: op.completion.clone().ok_or(Error::Invalid)?,
                peer,
            }));
        }
        Ok(None)
    }
    /// Authorize one explicit original-key Messaging retransmission, without a fresh hold, dispatch or current-price check.
    /// # Errors
    /// Noneligible/local-only work, absent original dispatch, foreign provider and unsupported family permission cannot be retried.
    pub fn retry_requester(
        &self,
        context: &Context<'_>,
        observation: &Context<'_>,
        original: OperationKey,
        peer: Peer,
    ) -> Result<Retry, Error> {
        tracing::trace!("Authorizing explicit family retry without repeating ordinary work");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        context.check(
            &self.config.authority,
            self.config.local,
            Action::Request,
            0,
        )?;
        self.check(observation, Action::Observe, &state)?;
        let op = state.records.get(&original).ok_or(Error::Denied)?;
        if op.service != "cr2se.messaging" || op.version != 1 || op.check.is_some() {
            return Err(Error::Unsupported);
        }
        if peer.local != self.config.local
            || op.terms.requester != self.config.local
            || peer.remote != op.terms.provider
            || peer.key.requester != self.config.local
            || peer.key.stream == 0
            || peer.remote == peer.local
            || peer.key == original
        {
            return Err(Error::Denied);
        }
        if !matches!(
            op.phase,
            Phase::Executing | Phase::Staged | Phase::Prepared | Phase::Commit | Phase::Committed
        ) {
            return Err(Error::Conflict);
        }
        let input = op.input.clone().ok_or(Error::Invalid)?;
        let pin = state.pins.get(&original).ok_or(Error::Unavailable)?;
        let id = eligible(pin.requester(), &input)?.ok_or(Error::Unsupported)?;
        Ok(Retry {
            domain: Arc::downgrade(&self.domain),
            original,
            peer,
            invocation: op.invocation(self.config.limits)?,
            input,
            id,
            maximum: op.terms.spending_bound,
            payment: Payment {
                transfer: op.terms.transfer,
                fee: op.terms.fee,
            },
        })
    }
    /// Apply a normally completed result on an explicit new-stream retry to only its original financial operation.
    /// # Errors
    /// Foreign owner/stream, changed family request, conflicting output and durable uncertainty cannot create a new payment.
    pub fn receive_replay(
        &self,
        context: &Context<'_>,
        retry: &Retry,
        observed: Peer,
        result: RemoteResult,
    ) -> Result<(), Error> {
        tracing::trace!("Applying family retry output to its original operation at most once");
        if !Arc::ptr_eq(&retry.domain.upgrade().ok_or(Error::Denied)?, &self.domain)
            || observed.local != retry.peer.local
            || observed.remote != retry.peer.remote
            || observed.key != retry.peer.key
        {
            return Err(Error::Denied);
        }
        {
            let state = self.state.lock().map_err(|_| Error::Unavailable)?;
            self.check(context, Action::Coordinate, &state)?;
            let op = state.records.get(&retry.original).ok_or(Error::Denied)?;
            if op.terms.requester != self.config.local
                || op.terms.provider != observed.remote
                || op.input.as_ref() != Some(&retry.input)
                || eligible(
                    state
                        .pins
                        .get(&retry.original)
                        .ok_or(Error::Unavailable)?
                        .requester(),
                    &retry.input,
                )? != Some(retry.id)
            {
                return Err(Error::Denied);
            }
        }
        // Correlation is remapped only inside the original owner after validating the explicit new-stream capability.
        self.receive(
            context,
            Peer {
                key: retry.original,
                ..observed
            },
            result,
        )
    }
}
// A family's opt-in cannot enable generic replay or substitute another operation's identifier.
fn eligible(
    requester: &dyn crate::modules::services::handler::Requester,
    input: &Value,
) -> Result<Option<Id>, Error> {
    use crate::modules::services::replay::Purpose;
    tracing::trace!("Checking explicit family-authorized replay purpose");
    let Some(id) = requester.cached_retry(input)? else {
        return Ok(None);
    };
    let Value::Object(fields) = input else {
        return Err(Error::Invalid);
    };
    let operation = fields
        .iter()
        .find(|(n, _)| n == "operation")
        .map(|(_, v)| v);
    let (name, kind) = match id.purpose {
        Purpose::Recovery => ("recoveryId", "recover"),
        Purpose::Removal => ("removalId", "remove"),
    };
    if !fields.iter().any(|(n, v)| {
        n == "placementId" && matches!(v,Value::Bytes(b) if b.as_slice()==id.placement)
    }) || operation != Some(&Value::String(kind.into()))
        || !fields
            .iter()
            .any(|(n, v)| n == name && matches!(v,Value::Bytes(b) if b.as_slice()==id.bytes))
    {
        return Err(Error::Invalid);
    }
    Ok(Some(id))
}
