//! Accepted binding, reservation and complete input admission.
use super::*;

impl<P: Persistence> Coordinator<P> {
    /// Retain exact accepted terms before obtaining one finite Ledger hold under the original operation key.
    /// # Errors
    /// Wrong local role, conflicting reuse, fee overflow, insufficient capacity and uncertain persistence fail explicitly.
    pub fn admit(
        &self,
        context: &Context<'_>,
        accepted: Accepted,
        payment: Payment,
    ) -> Result<OperationKey, Error> {
        tracing::trace!("Durably binding accepted invocation before Ledger reservation");
        let auth = &accepted.assessed.authorized;
        if auth.peer.local != self.config.local {
            return Err(Error::Denied);
        }
        let selection = &auth.selection;
        let o = selection.offering();
        let (requester, provider, issuer) = selection.roles();
        if accepted.price == crate::modules::services::pricing::Price::Included {
            return Err(Error::Invalid);
        }
        let maximum = accepted.price.maximum(&o.service, o.version)?;
        if maximum
            .checked_add(payment.fee.unwrap_or(0))
            .ok_or(Error::Limit)?
            > accepted.maximum
        {
            return Err(Error::Insufficient);
        }
        let accepted_bytes = codec::encode(
            &Value::Array(vec![
                Value::Bytes(accepted.bytes.bytes().to_vec()),
                Value::Bytes(selection.definition().document.bytes().to_vec()),
                Value::Bytes(o.source.to_string().into_bytes()),
                Value::Bytes(selection.publisher().0.to_vec()),
                Value::Bool(o.direction == crate::modules::board::offering::Direction::Wanted),
            ]),
            Budget {
                depth: 31,
                nodes: self.config.limits.nodes,
                bytes: self.config.limits.state_bytes,
            },
            self.config.limits.state_bytes,
        )?;
        let key = auth.peer.key;
        let operation = Operation {
            key,
            service: o.service.clone(),
            version: o.version,
            terms: Terms {
                requester,
                provider,
                issuer,
                maximum,
                fixed: matches!(
                    accepted.price,
                    crate::modules::services::pricing::Price::Fixed(_)
                ),
                accepted: accepted_bytes,
                transfer: payment.transfer,
                fee: payment.fee,
                spending_bound: accepted.maximum,
            },
            definition: selection.definition().document.bytes().to_vec(),
            metadata: accepted.assessed.assessment.metadata.clone(),
            pricing: accepted.price,
            check: None,
            input_bytes: accepted.assessed.assessment.input_bytes,
            factors: accepted.assessed.assessment.factors.clone(),
            phase: Phase::Accepted,
            input: None,
            completion: None,
            handler: None,
        };
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        if state
            .records
            .values()
            .any(|o| matches!(o.phase, Phase::Commit | Phase::Abort))
        {
            return Err(Error::Unknown);
        }
        let installed = if let Some(pin) = state.pins.get(&key) {
            pin.clone()
        } else {
            self.registry
                .select(&o.service, o.version, self.config.local == provider)?
        };
        if !installed.same_implementation(&auth.implementation) {
            return Err(Error::Denied);
        }
        if let Some(old) = state.records.get(&key) {
            if old.terms != operation.terms
                || old.definition != operation.definition
                || old.metadata != operation.metadata
                || old.pricing != operation.pricing
                || old.input_bytes != operation.input_bytes
                || old.factors != operation.factors
            {
                return Err(Error::Conflict);
            }
        } else {
            if state.records.len() >= self.config.limits.operations {
                return Err(Error::Limit);
            }
            if self.accounting.status(context, key)?.is_some() {
                // Missing coordinator history is not authorization to reuse a financial replay guard.
                return Err(Error::Conflict);
            }
            self.save(context, &mut state, operation)?;
            state.pins.insert(key, auth.implementation.clone());
        }
        drop(state);
        self.reserve(context, key)?;
        Ok(key)
    }
    /// Retry only original-key reservation after lost local acknowledgement, never service execution.
    /// # Errors
    /// Refuses absent/conflicting state, exhausted funds, revoked authority and unresolved journal/participant outcomes.
    pub fn reserve(&self, context: &Context<'_>, key: OperationKey) -> Result<(), Error> {
        tracing::trace!("Reserving exact retained Services invocation capacity");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if op.phase != Phase::Accepted {
            return if op.phase == Phase::Reserved {
                Ok(())
            } else {
                Err(Error::Conflict)
            };
        }
        if !op
            .check
            .as_ref()
            .is_some_and(crate::modules::services::checks::Info::included)
        {
            self.accounting.reserve(context, key, &op.terms)?;
        }
        op.phase = Phase::Reserved;
        self.save(context, &mut state, op)
    }
    /// Retain only complete family-validated input within the original assessed byte bound.
    /// No execution is performed, and exact local delivery retries reuse the retained input.
    /// # Errors
    /// Invalid or changed content, missing holds, wrong phase and caller/storage failures reject admission.
    pub fn supply_input(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        input: Value,
    ) -> Result<(), Error> {
        tracing::trace!("Retaining complete validated input before service execution");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if matches!(op.phase, Phase::Abort | Phase::Aborted) {
            return Err(Error::Conflict);
        }
        if let Some(original) = &op.input {
            return if original == &input {
                Ok(())
            } else {
                Err(Error::Conflict)
            };
        }
        if op.phase != Phase::Reserved {
            return Err(Error::Conflict);
        }
        let budget = Budget {
            depth: 31,
            nodes: self.config.limits.nodes,
            bytes: usize::try_from(op.input_bytes)
                .map_err(|_| Error::Limit)?
                .min(self.config.limits.state_bytes),
        };
        let requester = state.pins.get(&key).ok_or(Error::Unavailable)?.requester();
        if op.check.is_some() {
            input.bounded(budget)?;
            if input != op.metadata {
                return Err(Error::Invalid);
            }
            requester.checks().ok_or(Error::Unsupported)?.challenge(
                &op.check_invocation(self.config.limits, context.deadline)?,
                &input,
            )?;
        } else {
            let invocation = op.invocation(self.config.limits)?;
            crate::modules::services::validation::complete(
                &input,
                &invocation.definition.document.value()["input"],
                budget,
                requester,
            )?;
            requester.validate_input(&op.metadata, &input, budget)?;
        }
        op.input = Some(input);
        self.save(context, &mut state, op)
    }
}
