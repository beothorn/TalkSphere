//! Original-key status recovery without work re-execution.
use super::*;

impl<P: Persistence> Coordinator<P> {
    /// List bounded nonterminal provider work for this installed contract before opening admission.
    /// # Errors
    /// Requires coordinator authority and an authoritative recovered Services image.
    pub fn pending_provider(
        &self,
        context: &Context<'_>,
        service: &str,
        version: u32,
    ) -> Result<Vec<OperationKey>, Error> {
        tracing::trace!("Enumerating original unresolved local provider work for recovery");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        Ok(state
            .records
            .values()
            .filter(|op| {
                op.terms.provider == self.config.local
                    && op.service == service
                    && op.version == version
                    && !matches!(op.phase, Phase::Committed | Phase::Aborted)
            })
            .map(|op| op.key)
            .collect())
    }

    /// Recover a local provider operation for which the authoritative family store has no record.
    /// Ordinary work with a retained result or commit decision requires its original family record.
    /// A closed family admission gate and recovered owner are required before calling.
    /// # Errors
    /// Conflicting scope or missing required participant state fails closed.
    pub fn recover_absent_provider(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        service: &str,
        version: u32,
    ) -> Result<Phase, Error> {
        tracing::trace!("Reconciling provider work with no durable family effect");
        let (phase, check) = {
            let state = self.state.lock().map_err(|_| Error::Unavailable)?;
            self.check(context, Action::Coordinate, &state)?;
            let op = state.records.get(&key).ok_or(Error::Denied)?;
            if op.terms.provider != self.config.local
                || op.service != service
                || op.version != version
            {
                return Err(Error::Denied);
            }
            (op.phase, op.check.is_some())
        };
        match phase {
            Phase::Accepted | Phase::Reserved | Phase::Executing => {
                self.decide(context, key, Decision::Abort)?;
                self.abort(context, key)?;
                Ok(Phase::Aborted)
            }
            Phase::Abort => self.resume(context, key),
            Phase::Staged | Phase::Prepared | Phase::Commit if check => self.resume(context, key),
            _ => Err(Error::Unknown),
        }
    }

    /// Resume original durable local transitions after authoritative reopen, never execution or peer invocation.
    /// Reserved work still needs explicit input/start; uncertain execution needs a recoverable completion.
    /// # Errors
    /// Propagates dependency uncertainty and requires explicit coordinator authority for decision delivery.
    pub fn resume(&self, context: &Context<'_>, key: OperationKey) -> Result<Phase, Error> {
        tracing::trace!("Resuming original durable Services decisions without executing again");
        for _ in 0..9 {
            let phase = {
                let state = self.state.lock().map_err(|_| Error::Unavailable)?;
                self.check(context, Action::Coordinate, &state)?;
                state.records.get(&key).ok_or(Error::Denied)?.phase
            };
            match phase {
                Phase::Accepted => self.reserve(context, key)?,
                Phase::Reserved => return Err(Error::Unknown),
                Phase::Executing => self.stage(context, key)?,
                Phase::Staged => self.prepare(context, key)?,
                Phase::Prepared => self.decide(context, key, Decision::Commit)?,
                Phase::Commit => self.commit(context, key)?,
                Phase::Abort => self.abort(context, key)?,
                Phase::Committed | Phase::Aborted => return Ok(phase),
            }
        }
        Err(Error::Unknown)
    }
}
