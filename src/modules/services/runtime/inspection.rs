//! Owned results, local verification and lifecycle observations.
use super::*;

impl<P: Persistence> Coordinator<P> {
    /// Return a retained complete result only after all local effects are durably acknowledged.
    /// Payment completion and remote receipt remain independent of this local outcome.
    /// # Errors
    /// Pending/uncertain work returns Unknown; aborted work conflicts and authority remains required.
    pub fn result(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<crate::modules::services::handler::Completion, Error> {
        tracing::trace!("Reading locally committed retained Services result");
        self.completed(context, key)
            .map(|(_, completion)| completion)
    }
    /// Return immutable original invocation and complete output only after all local commitments are acknowledged.
    /// # Errors
    /// Pending/uncertain work returns Unknown; checks, aborted work, malformed retained state and foreign authority fail explicitly.
    pub fn completed(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<
        (
            crate::modules::services::handler::Invocation,
            crate::modules::services::handler::Completion,
        ),
        Error,
    > {
        tracing::trace!("Reading committed invocation and output for original-stream transmission");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Observe, &state)?;
        let op = state.records.get(&key).ok_or(Error::Denied)?;
        if op.check.is_some() {
            return Err(Error::Unsupported);
        }
        if matches!(op.phase, Phase::Abort | Phase::Aborted) {
            return Err(Error::Conflict);
        }
        if op.phase != Phase::Committed {
            return Err(Error::Unknown);
        }
        let completion = op.completion.clone().ok_or(Error::Invalid)?;
        Ok((op.invocation(self.config.limits)?, completion))
    }
    /// Run the installed requester-local verifier on original retained evidence without spending or refunds.
    /// Private expectations are borrowed for this call and never persisted or sent to a provider.
    /// # Errors
    /// Authority, malformed input and unusable contracts remain explicit; unavailable evidence is Inconclusive.
    pub fn verify(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        expectations: Option<&Value>,
    ) -> Result<crate::modules::services::handler::Check, Error> {
        tracing::trace!("Running requester-local verification without financial side effects");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Observe, &state)?;
        let op = state.records.get(&key).ok_or(Error::Denied)?;
        if op.check.is_some() {
            return Err(Error::Unsupported);
        }
        if op.phase != Phase::Committed {
            return Ok(crate::modules::services::handler::Check::Inconclusive);
        }
        let bytes = expectations
            .map(|v| {
                v.byte_len(Budget {
                    depth: 31,
                    nodes: self.config.limits.nodes,
                    bytes: self.config.limits.state_bytes,
                })
            })
            .transpose()?
            .unwrap_or(0);
        context.check(
            &self.config.authority,
            self.config.local,
            Action::Observe,
            bytes,
        )?;
        let Some(input) = &op.input else {
            return Ok(crate::modules::services::handler::Check::Inconclusive);
        };
        let Some(completion) = &op.completion else {
            return Ok(crate::modules::services::handler::Check::Inconclusive);
        };
        let accepted = crate::modules::services::handler::Blob::new(
            op.terms.accepted.clone(),
            self.config.limits.state_bytes,
        )?;
        match state
            .pins
            .get(&key)
            .ok_or(Error::Unavailable)?
            .requester()
            .verify(input, &accepted, &completion.output, expectations)
        {
            Err(Error::Unavailable | Error::Unknown) => {
                Ok(crate::modules::services::handler::Check::Inconclusive)
            }
            result => result,
        }
    }
    /// Observe durable local lifecycle without disclosing payloads or assuming payment completion.
    /// # Errors
    /// Requires Observe authority and reconciled complete coordinator state.
    pub fn status(&self, context: &Context<'_>, key: OperationKey) -> Result<Option<Phase>, Error> {
        tracing::trace!("Observing durable Services invocation lifecycle");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Observe, &state)?;
        Ok(state.records.get(&key).map(|o| o.phase))
    }
}
