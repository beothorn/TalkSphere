//! Once-only dispatch and complete result staging.
use super::*;

impl<P: Persistence> Coordinator<P> {
    /// Persist one execution-attempt marker before dispatching the already reserved complete input.
    /// Success here means dispatch completed, not validated service success or payment.
    /// # Errors
    /// An Executing retry returns Unknown and never re-executes; wrong phase/input, dependency or storage failures propagate.
    pub fn start(&self, context: &Context<'_>, key: OperationKey) -> Result<(), Error> {
        tracing::trace!("Journaling service execution attempt before dispatch");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if op.phase == Phase::Executing {
            return Err(Error::Unknown);
        }
        if matches!(
            op.phase,
            Phase::Staged | Phase::Prepared | Phase::Commit | Phase::Committed
        ) {
            return Ok(());
        }
        if op.phase != Phase::Reserved {
            return Err(Error::Conflict);
        }
        if state
            .records
            .values()
            .any(|o| matches!(o.phase, Phase::Commit | Phase::Abort))
        {
            return Err(Error::Unknown);
        }
        if op.terms.provider != self.config.local {
            return Err(Error::Unsupported);
        }
        let input = op.input.clone().ok_or(Error::Invalid)?;
        if let Some(info) = &op.check {
            let deadline = context.deadline.min(
                std::time::Instant::now()
                    .checked_add(std::time::Duration::from_millis(info.response))
                    .ok_or(Error::Limit)?,
            );
            let invocation = op.check_invocation(self.config.limits, deadline)?;
            state
                .pins
                .get(&key)
                .ok_or(Error::Unavailable)?
                .requester()
                .checks()
                .ok_or(Error::Unsupported)?
                .challenge(&invocation, &input)?;
            op.phase = Phase::Executing;
            self.save(context, &mut state, op.clone())?;
            let evidence = state
                .pins
                .get(&key)
                .ok_or(Error::Unavailable)?
                .provider()?
                .evidence(&invocation, &input)?;
            let bounded = Context {
                version: context.version,
                grant: context.grant,
                deadline,
                cancelled: context.cancelled,
            };
            return self.retain_check(&bounded, &mut state, op, evidence);
        }
        let invocation = op.invocation(self.config.limits)?;
        op.phase = Phase::Executing;
        self.save(context, &mut state, op)?;
        state
            .pins
            .get(&key)
            .ok_or(Error::Unavailable)?
            .provider()?
            .execute(&invocation, &input)
            .map(|_| ())
    }
    /// Recover a staged completion, validate it against immutable terms and retain its full bytes durably.
    /// No service execution is retried and no settlement occurs in this stage.
    /// # Errors
    /// Invalid output, quoted sizes, final price, absent completion and persistence uncertainty reject success.
    pub fn stage(&self, context: &Context<'_>, key: OperationKey) -> Result<(), Error> {
        tracing::trace!("Recovering original local provider completion for durable staging");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if matches!(
            op.phase,
            Phase::Staged | Phase::Prepared | Phase::Commit | Phase::Committed
        ) {
            return Ok(());
        }
        if op.phase != Phase::Executing {
            return Err(Error::Conflict);
        }
        if op.check.is_some() || op.terms.provider != self.config.local {
            return Err(Error::Unknown);
        }
        let completion = state
            .pins
            .get(&key)
            .ok_or(Error::Unavailable)?
            .provider()?
            .completion(key)?;
        self.retain(context, &mut state, op, completion)
    }
    // Both local and remote result paths independently enforce schema, contextual terms and exact final price.
    pub(super) fn retain(
        &self,
        context: &Context<'_>,
        state: &mut State,
        mut op: Operation,
        completion: crate::modules::services::handler::Completion,
    ) -> Result<(), Error> {
        tracing::trace!("Validating complete service result before durable retention");
        let pin = state.pins.get(&op.key).ok_or(Error::Unavailable)?;
        let invocation = op.invocation(self.config.limits)?;
        let budget = Budget {
            depth: 31,
            nodes: self.config.limits.nodes,
            bytes: self.config.limits.state_bytes,
        };
        crate::modules::services::validation::complete(
            &completion.output,
            &invocation.definition.document.value()["output"],
            budget,
            pin.requester(),
        )?;
        pin.requester().validate_result(
            &invocation,
            op.input.as_ref().ok_or(Error::Invalid)?,
            &completion.output,
        )?;
        crate::modules::services::pricing::validate_output_price(
            &completion.output,
            completion.price,
        )?;
        if completion.effect.bytes().is_empty() {
            return Err(Error::Invalid);
        }
        if completion.result.bytes().len() > self.config.limits.state_bytes
            || completion.effect.bytes().len() > self.config.limits.state_bytes
        {
            return Err(Error::Limit);
        }
        let billed = if matches!(
            op.pricing,
            crate::modules::services::pricing::Price::Computation { .. }
        ) {
            let Value::Object(fields) = &completion.output else {
                return Err(Error::Invalid);
            };
            let Some((_, Value::Uint64(billed))) =
                fields.iter().find(|(n, _)| n == "billedMilliseconds")
            else {
                return Err(Error::Invalid);
            };
            Some(*billed)
        } else {
            None
        };
        if op.pricing.final_price(&op.service, op.version, billed)? != completion.price {
            return Err(Error::Invalid);
        }
        op.completion = Some(completion);
        op.phase = Phase::Staged;
        self.save(context, state, op)
    }
    /// Retain the outgoing attempt before returning complete invocation data to the trusted peer adapter.
    /// Repeated calls never resend accepted work after an ambiguous local reply.
    /// # Errors
    /// Requires a local requester with complete input; uncertain/pending decisions and repeated dispatch return Unknown.
    pub fn outbound(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<(crate::modules::services::handler::Invocation, Value), Error> {
        tracing::trace!("Journaling requester dispatch before returning outbound invocation data");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if op.check.is_some() {
            return Err(Error::Unsupported);
        }
        if op.terms.requester != self.config.local || op.terms.provider == self.config.local {
            return Err(Error::Denied);
        }
        if op.phase == Phase::Executing {
            return Err(Error::Unknown);
        }
        if op.phase != Phase::Reserved {
            return Err(Error::Conflict);
        }
        if state
            .records
            .values()
            .any(|o| matches!(o.phase, Phase::Commit | Phase::Abort))
        {
            return Err(Error::Unknown);
        }
        let input = op.input.clone().ok_or(Error::Invalid)?;
        let invocation = op.invocation(self.config.limits)?;
        op.phase = Phase::Executing;
        self.save(context, &mut state, op)?;
        Ok((invocation, input))
    }
    /// Retain a complete successful result from the authenticated original provider and stream.
    /// # Errors
    /// Wrong provenance, changed replayed result, invalid output/price and storage failures preserve the original attempt.
    pub fn receive(
        &self,
        context: &Context<'_>,
        peer: crate::modules::services::handler::Peer,
        result: RemoteResult,
    ) -> Result<(), Error> {
        use sha2::{Digest, Sha256};
        tracing::trace!("Validating authenticated original-provider result event");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let op = state.records.get(&peer.key).cloned().ok_or(Error::Denied)?;
        if op.check.is_some() {
            return Err(Error::Unsupported);
        }
        if peer.local != self.config.local
            || op.terms.requester != self.config.local
            || op.terms.provider == self.config.local
            || peer.remote != op.terms.provider
        {
            return Err(Error::Denied);
        }
        if let Some(original) = &op.completion {
            return if original.output == result.output
                && original.result == result.bytes
                && original.price == result.price
            {
                Ok(())
            } else {
                Err(Error::Conflict)
            };
        }
        if op.phase != Phase::Executing {
            return Err(Error::Conflict);
        }
        let effect = crate::modules::services::handler::Blob::new(
            Sha256::digest(result.bytes.bytes()).to_vec(),
            32,
        )?;
        self.retain(
            context,
            &mut state,
            op,
            crate::modules::services::handler::Completion {
                output: result.output,
                result: result.bytes,
                price: result.price,
                effect,
            },
        )
    }
    /// Record an explicit complete original-provider failure, never inferred from timeout or disconnect.
    /// # Errors
    /// Wrong peer/stream and failure after a retained successful result are denied or conflicting.
    pub fn decline(
        &self,
        context: &Context<'_>,
        peer: crate::modules::services::handler::Peer,
    ) -> Result<(), Error> {
        tracing::trace!("Recording explicit original-provider uncommitted failure");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&peer.key).cloned().ok_or(Error::Denied)?;
        if peer.local != self.config.local
            || op.terms.requester != self.config.local
            || peer.remote != op.terms.provider
            || op.terms.provider == self.config.local
        {
            return Err(Error::Denied);
        }
        if matches!(op.phase, Phase::Abort | Phase::Aborted) {
            return Ok(());
        }
        if !matches!(
            op.phase,
            Phase::Accepted | Phase::Reserved | Phase::Executing
        ) || op.completion.is_some()
        {
            return Err(Error::Conflict);
        }
        op.phase = Phase::Abort;
        self.save(context, &mut state, op)
    }
}
