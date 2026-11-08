//! Original-key status recovery without work re-execution.
use super::*;

impl<P: Persistence> Coordinator<P> {
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
