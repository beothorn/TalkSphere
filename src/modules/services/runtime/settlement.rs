//! Immutable local decisions and participant delivery.
use super::*;

impl<P: Persistence> Coordinator<P> {
    /// Durably prepare the pinned family and exact Ledger charge, retaining tokens before a decision.
    /// # Errors
    /// Changed/invalid tokens, participant uncertainty and storage failures retain original holds for recovery.
    pub fn prepare(&self, context: &Context<'_>, key: OperationKey) -> Result<(), Error> {
        tracing::trace!("Preparing exact family and Ledger effects before coordinator decision");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if matches!(op.phase, Phase::Prepared | Phase::Commit | Phase::Committed) {
            return Ok(());
        }
        if op.phase != Phase::Staged {
            return Err(Error::Conflict);
        }
        let completion = op.completion.clone().ok_or(Error::Invalid)?;
        if op.check.is_none() && op.terms.provider == self.config.local && op.handler.is_none() {
            let token = state
                .pins
                .get(&key)
                .ok_or(Error::Unavailable)?
                .provider()?
                .prepare(key, &completion.effect)?;
            if token.bytes().is_empty() || token.bytes().len() > self.config.limits.state_bytes {
                return Err(Error::Invalid);
            }
            op.handler = Some(token);
            self.save(context, &mut state, op.clone())?;
        }
        if !op
            .check
            .as_ref()
            .is_some_and(crate::modules::services::checks::Info::included)
        {
            self.accounting.prepare(
                context,
                key,
                &op.terms,
                completion.price,
                &op.reference(self.config.limits)?,
            )?;
        }
        op.phase = Phase::Prepared;
        self.save(context, &mut state, op)
    }
    /// Persist one immutable local decision; commit requires every participant prepared and retained valid output.
    /// # Errors
    /// Premature commit, contradictory decisions, missing bindings and caller/storage failures are explicit errors.
    pub fn decide(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        decision: Decision,
    ) -> Result<(), Error> {
        tracing::trace!("Persisting immutable Services coordinator decision");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if decision == Decision::Abort
            && op.terms.requester == self.config.local
            && op.terms.provider != self.config.local
            && matches!(op.phase, Phase::Executing | Phase::Staged | Phase::Prepared)
        {
            return Err(Error::Unknown);
        }
        match (op.phase, decision) {
            (Phase::Commit | Phase::Committed, Decision::Commit)
            | (Phase::Abort | Phase::Aborted, Decision::Abort) => return Ok(()),
            (Phase::Commit | Phase::Committed | Phase::Abort | Phase::Aborted, _) => {
                return Err(Error::Conflict);
            }
            (Phase::Prepared, Decision::Commit) => {
                if op.completion.is_none()
                    || (op.check.is_none()
                        && op.terms.provider == self.config.local
                        && op.handler.is_none())
                {
                    return Err(Error::Invalid);
                }
                op.phase = Phase::Commit;
            }
            (_, Decision::Commit) => return Err(Error::Conflict),
            (_, Decision::Abort) => op.phase = Phase::Abort,
        }
        self.save(context, &mut state, op)
    }
    /// Deliver an already recorded commit to every prepared participant without repeating execution.
    /// # Errors
    /// Missing/contradictory decisions and dependency loss preserve pending commitment; success requires all acknowledgements.
    pub fn commit(&self, context: &Context<'_>, key: OperationKey) -> Result<(), Error> {
        tracing::trace!("Delivering recorded Services commit to all local participants");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if op.phase == Phase::Committed {
            return Ok(());
        }
        if op.phase != Phase::Commit {
            return Err(Error::Conflict);
        }
        let completion = op.completion.as_ref().ok_or(Error::Invalid)?;
        let provider = if op.check.is_none() && op.terms.provider == self.config.local {
            Some(state.pins.get(&key).ok_or(Error::Unavailable)?.provider()?)
        } else {
            None
        };
        if let Some(provider) = provider {
            let token = op.handler.as_ref().ok_or(Error::Invalid)?;
            match provider.status(key)? {
                crate::modules::services::handler::Status::Committed => {}
                crate::modules::services::handler::Status::Prepared(original)
                    if &original == token =>
                {
                    provider.commit(key, token)?
                }
                _ => return Err(Error::Unknown),
            }
        }
        if !op
            .check
            .as_ref()
            .is_some_and(crate::modules::services::checks::Info::included)
        {
            match self.accounting.status(context, key)? {
                Some(crate::modules::ledger::reservation::Status::Committed) => {}
                Some(crate::modules::ledger::reservation::Status::Prepared) => {
                    self.accounting.commit(
                        context,
                        key,
                        &op.terms,
                        completion.price,
                        &op.reference(self.config.limits)?,
                    )?
                }
                _ => return Err(Error::Unknown),
            }
        }
        if let Some(provider) = provider
            && !matches!(
                provider.status(key)?,
                crate::modules::services::handler::Status::Committed
            )
        {
            return Err(Error::Unknown);
        }
        if !op
            .check
            .as_ref()
            .is_some_and(crate::modules::services::checks::Info::included)
            && self.accounting.status(context, key)?
                != Some(crate::modules::ledger::reservation::Status::Committed)
        {
            return Err(Error::Unknown);
        }
        op.phase = Phase::Committed;
        self.save(context, &mut state, op)
    }
    /// Deliver only an explicit recorded abort, releasing uncommitted resources without a charge.
    /// # Errors
    /// Commit decisions cannot be replaced; unavailable participants retain the abort decision for recovery.
    pub fn abort(&self, context: &Context<'_>, key: OperationKey) -> Result<(), Error> {
        tracing::trace!("Delivering explicit Services abort without erasing committed obligations");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if op.phase == Phase::Aborted {
            return Ok(());
        }
        if op.phase != Phase::Abort {
            return Err(Error::Conflict);
        }
        if op.check.is_none() && op.terms.provider == self.config.local {
            let provider = state.pins.get(&key).ok_or(Error::Unavailable)?.provider()?;
            let token = if op.handler.is_some() {
                op.handler.clone()
            } else {
                match provider.status(key)? {
                    crate::modules::services::handler::Status::Prepared(token) => Some(token),
                    crate::modules::services::handler::Status::Committed => {
                        return Err(Error::Conflict);
                    }
                    _ => None,
                }
            };
            provider.abort(key, token.as_ref())?;
        }
        if !op
            .check
            .as_ref()
            .is_some_and(crate::modules::services::checks::Info::included)
        {
            let reference = if op.completion.is_some() {
                Some(op.reference(self.config.limits)?)
            } else {
                None
            };
            let effect = match (
                self.accounting.status(context, key)?,
                op.completion.as_ref(),
                reference.as_ref(),
            ) {
                (Some(crate::modules::ledger::reservation::Status::Prepared), Some(c), Some(r)) => {
                    Some((c.price, r.as_slice()))
                }
                (Some(crate::modules::ledger::reservation::Status::Committed), _, _) => {
                    return Err(Error::Conflict);
                }
                _ => None,
            };
            self.accounting.abort(context, key, &op.terms, effect)?;
        }
        op.phase = Phase::Aborted;
        self.save(context, &mut state, op)
    }
}
