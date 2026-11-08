//! Evidence-check admission and requester outcomes share the coordinator's durable operation namespace.
use super::*;
use crate::modules::services::checks::{
    Agreement, Info, Invocation as CheckInvocation, Mode, Stamp, Timing,
};
use sha2::{Digest, Sha256};

impl<P: Persistence> Coordinator<P> {
    /// Admit explicitly accepted remote evidence terms without exposing requester expectations.
    /// # Errors
    /// Local-only checks, wrong quote/provenance, unknown rules, exhausted entitlements and monetary bounds fail before execution.
    pub fn admit_check(
        &self,
        context: &Context<'_>,
        agreement: Agreement,
        current: &dyn crate::modules::services::quote::Current,
    ) -> Result<OperationKey, Error> {
        tracing::trace!("Admitting exact paid or included evidence-check consent");
        context.check(
            &self.config.authority,
            self.config.local,
            Action::Coordinate,
            agreement.bytes.bytes().len(),
        )?;
        if agreement.bytes.bytes().len() > self.config.limits.state_bytes {
            return Err(Error::Limit);
        }
        let auth = &agreement.authorized;
        let selection = &auth.selection;
        let o = selection.offering();
        let (requester, provider, issuer) = selection.roles();
        let key = auth.peer.key;
        if auth.peer.local != self.config.local
            || agreement.bytes.bytes().is_empty()
            || <[u8; 32]>::from(Sha256::digest(agreement.bytes.bytes())) != agreement.hash
        {
            return Err(Error::Denied);
        }
        if requester == self.config.local && auth.input.as_ref() != Some(&agreement.challenge) {
            return Err(Error::Denied);
        }
        agreement.challenge.bounded(Budget {
            depth: 31,
            nodes: self.config.limits.nodes,
            bytes: self.config.limits.state_bytes,
        })?;
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
        let rules = installed.requester().checks().ok_or(Error::Unsupported)?;
        let mode = rules.mode(o, &agreement.challenge)?;
        if mode == Mode::Local {
            return Err(Error::Unsupported);
        }
        let response = rules.response_milliseconds(o)?;
        if response == 0 {
            return Err(Error::Invalid);
        }
        let included = matches!(mode, Mode::Included { .. });
        let price = if included {
            0
        } else {
            crate::modules::board::terms::positive(&o.source["checkPrice"])
                .map_err(|_| Error::Invalid)?
        };
        if included && (agreement.payment.transfer.is_some() || agreement.payment.fee.is_some()) {
            return Err(Error::Invalid);
        }
        if price
            .checked_add(agreement.payment.fee.unwrap_or(0))
            .ok_or(Error::Limit)?
            > agreement.maximum
        {
            return Err(Error::Insufficient);
        }
        let info = Info {
            mode,
            response,
            sent: None,
            received: None,
        };
        let accepted = codec::encode(
            &Value::Array(vec![
                Value::Bytes(agreement.bytes.bytes().to_vec()),
                Value::Bytes(selection.definition().document.bytes().to_vec()),
                Value::Bytes(o.source.to_string().into_bytes()),
                Value::Bytes(selection.publisher().0.to_vec()),
                Value::Bool(o.direction == crate::modules::board::offering::Direction::Wanted),
                info.binding(),
            ]),
            Budget {
                depth: 31,
                nodes: self.config.limits.nodes,
                bytes: self.config.limits.state_bytes,
            },
            self.config.limits.state_bytes,
        )?;
        let op = Operation {
            key,
            service: o.service.clone(),
            version: o.version,
            terms: Terms {
                requester,
                provider,
                issuer,
                maximum: price,
                fixed: true,
                accepted,
                transfer: agreement.payment.transfer,
                fee: agreement.payment.fee,
                spending_bound: agreement.maximum,
            },
            definition: selection.definition().document.bytes().to_vec(),
            metadata: agreement.challenge,
            pricing: if included {
                crate::modules::services::pricing::Price::Included
            } else {
                crate::modules::services::pricing::Price::Fixed(price)
            },
            check: Some(info),
            input_bytes: self.config.limits.state_bytes as u64,
            factors: Value::Object(vec![]),
            phase: Phase::Accepted,
            input: None,
            completion: None,
            handler: None,
        };
        rules.shape(&op.metadata)?;
        rules.challenge(
            &op.check_invocation(self.config.limits, context.deadline)?,
            &op.metadata,
        )?;
        current.check(selection)?;
        context.check(
            &self.config.authority,
            self.config.local,
            Action::Coordinate,
            agreement.bytes.bytes().len(),
        )?;
        if let Some(original) = state.records.get(&key) {
            if original.terms != op.terms
                || original.metadata != op.metadata
                || original.check.as_ref().map(Info::binding)
                    != op.check.as_ref().map(Info::binding)
            {
                return Err(Error::Conflict);
            }
        } else {
            if state.records.len() >= self.config.limits.operations {
                return Err(Error::Limit);
            }
            if let Mode::Included { scope, maximum } =
                &op.check.as_ref().ok_or(Error::Invalid)?.mode
            {
                if scope.bytes().is_empty() || *maximum == 0 {
                    return Err(Error::Invalid);
                }
                let mut used = 0u32;
                for old in state.records.values().filter(|old| {
                    old.terms.requester == requester && old.terms.provider == provider
                }) {
                    if let Some(Info {
                        mode:
                            Mode::Included {
                                scope: previous,
                                maximum: limit,
                            },
                        ..
                    }) = &old.check
                        && previous == scope
                    {
                        if maximum != limit {
                            return Err(Error::Conflict);
                        }
                        used = used.checked_add(1).ok_or(Error::Limit)?;
                    }
                }
                if used >= *maximum {
                    return Err(Error::Limit);
                }
            }
            self.save(context, &mut state, op)?;
            state.pins.insert(key, installed);
        }
        drop(state);
        self.reserve(context, key)?;
        Ok(key)
    }

    // Validate only defined provider evidence; correctness and private expectations are evaluated later by the requester.
    pub(super) fn retain_check(
        &self,
        context: &Context<'_>,
        state: &mut State,
        mut op: Operation,
        evidence: Value,
    ) -> Result<(), Error> {
        tracing::trace!("Durably retaining complete well-formed check evidence");
        evidence.bounded(Budget {
            depth: 31,
            nodes: self.config.limits.nodes,
            bytes: self.config.limits.state_bytes,
        })?;
        state
            .pins
            .get(&op.key)
            .ok_or(Error::Unavailable)?
            .requester()
            .checks()
            .ok_or(Error::Unsupported)?
            .evidence(&op.metadata, &evidence)?;
        let bytes = codec::encode(
            &evidence,
            Budget {
                depth: 31,
                nodes: self.config.limits.nodes,
                bytes: self.config.limits.state_bytes,
            },
            self.config.limits.state_bytes,
        )?;
        let effect =
            crate::modules::services::handler::Blob::new(Sha256::digest(&bytes).to_vec(), 32)?;
        op.completion = Some(crate::modules::services::handler::Completion {
            output: evidence,
            result: crate::modules::services::handler::Blob::new(
                bytes,
                self.config.limits.state_bytes,
            )?,
            price: op.terms.maximum,
            effect,
        });
        op.phase = Phase::Staged;
        self.save(context, state, op)
    }

    /// Journal one requester check dispatch before returning its challenge, without private expectations.
    /// # Errors
    /// Wrong role, incomplete input and duplicate/ambiguous dispatch fail without resending the check.
    pub fn outbound_check(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<(CheckInvocation, Value), Error> {
        tracing::trace!("Journaling requester evidence challenge before outbound delivery");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if op.terms.requester != self.config.local || op.terms.provider == self.config.local {
            return Err(Error::Denied);
        }
        if op.phase == Phase::Executing {
            return Err(Error::Unknown);
        }
        if op.phase != Phase::Reserved {
            return Err(Error::Conflict);
        }
        let input = op.input.clone().ok_or(Error::Invalid)?;
        let invocation = op.check_invocation(self.config.limits, context.deadline)?;
        if state
            .records
            .values()
            .any(|o| matches!(o.phase, Phase::Commit | Phase::Abort))
        {
            return Err(Error::Unknown);
        }
        op.phase = Phase::Executing;
        self.save(context, &mut state, op)?;
        Ok((invocation, input))
    }

    /// Record the trusted local instant when the complete challenge was sent, not when quoting started.
    /// # Errors
    /// Wrong role/phase, changed clock association and timestamp overflow reject timing updates.
    pub fn check_sent(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        stamp: Stamp,
    ) -> Result<(), Error> {
        tracing::trace!("Recording local complete-challenge send timing");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if op.terms.requester != self.config.local || op.phase != Phase::Executing {
            return Err(Error::Denied);
        }
        let info = op.check.as_mut().ok_or(Error::Unsupported)?;
        stamp
            .milliseconds
            .checked_add(info.response)
            .ok_or(Error::Limit)?;
        if let Some(old) = info.sent {
            return if old == stamp {
                Ok(())
            } else {
                Err(Error::Conflict)
            };
        }
        info.sent = Some(stamp);
        self.save(context, &mut state, op)
    }

    /// Accept complete evidence only on the authenticated original provider stream with locally observed timing.
    /// # Errors
    /// Wrong peer, malformed or changed evidence, known lateness and storage failure cannot authorize another charge.
    pub fn receive_check(
        &self,
        context: &Context<'_>,
        peer: crate::modules::services::handler::Peer,
        evidence: Value,
        stamp: Stamp,
    ) -> Result<(), Error> {
        tracing::trace!("Validating original-provider evidence and local receive timing");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&peer.key).cloned().ok_or(Error::Denied)?;
        if peer.local != self.config.local
            || op.terms.requester != self.config.local
            || peer.remote != op.terms.provider
        {
            return Err(Error::Denied);
        }
        if let Some(c) = &op.completion {
            return if c.output == evidence {
                Ok(())
            } else {
                Err(Error::Conflict)
            };
        }
        if op.phase != Phase::Executing {
            return Err(Error::Conflict);
        }
        let info = op.check.as_mut().ok_or(Error::Unsupported)?;
        if let Some(sent) = info.sent
            && sent.epoch == stamp.epoch
            && stamp.milliseconds
                > sent
                    .milliseconds
                    .checked_add(info.response)
                    .ok_or(Error::Limit)?
        {
            return Err(Error::Cancelled);
        }
        info.received = Some(stamp);
        self.retain_check(context, &mut state, op, evidence)
    }

    /// Resolve a locally observed check deadline with no complete valid evidence as uncharged and inconclusive.
    /// This specific check rule is not a generic rollback proof for remote service work.
    /// # Errors
    /// Wrong or incomparable clock/role, premature timeout and any retained valid success reject release.
    pub fn check_timeout(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        now: Stamp,
    ) -> Result<(), Error> {
        tracing::trace!("Recording uncharged local evidence timeout without fabricating rollback");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let mut op = state.records.get(&key).cloned().ok_or(Error::Denied)?;
        if op.terms.requester != self.config.local
            || op.phase != Phase::Executing
            || op.completion.is_some()
        {
            return Err(Error::Conflict);
        }
        let info = op.check.as_ref().ok_or(Error::Unsupported)?;
        let sent = info.sent.ok_or(Error::Unknown)?;
        if sent.epoch != now.epoch {
            return Err(Error::Unknown);
        }
        if now.milliseconds
            <= sent
                .milliseconds
                .checked_add(info.response)
                .ok_or(Error::Limit)?
        {
            return Err(Error::Conflict);
        }
        op.phase = Phase::Abort;
        self.save(context, &mut state, op)?;
        drop(state);
        self.abort(context, key)
    }

    /// Return retained provider evidence only after the local check operation is durably completed.
    /// # Errors
    /// Wrong purpose, missing evidence, pending outcomes and caller failures remain explicit.
    pub fn check_evidence(&self, context: &Context<'_>, key: OperationKey) -> Result<Value, Error> {
        tracing::trace!("Reading complete durably committed provider evidence");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Observe, &state)?;
        let op = state.records.get(&key).ok_or(Error::Denied)?;
        if op.check.is_none() {
            return Err(Error::Unsupported);
        }
        if op.phase != Phase::Committed {
            return Err(Error::Unknown);
        }
        Ok(op.completion.as_ref().ok_or(Error::Invalid)?.output.clone())
    }

    /// Calculate requester-only outcomes with private expectations, without refunds or trust changes.
    /// # Errors
    /// Wrong role/purpose and invalid caller data fail; missing evidence remains Inconclusive.
    pub fn check_outcome(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        expectations: Option<&Value>,
    ) -> Result<crate::modules::services::handler::Check, Error> {
        tracing::trace!("Calculating requester check outcome with privately held expectations");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Observe, &state)?;
        let op = state.records.get(&key).ok_or(Error::Denied)?;
        if op.terms.requester != self.config.local {
            return Err(Error::Denied);
        }
        let info = op.check.as_ref().ok_or(Error::Unsupported)?;
        if op.phase != Phase::Committed {
            return Ok(crate::modules::services::handler::Check::Inconclusive);
        }
        if let Some(expected) = expectations {
            let bytes = expected.byte_len(Budget {
                depth: 31,
                nodes: self.config.limits.nodes,
                bytes: self.config.limits.state_bytes,
            })?;
            context.check(
                &self.config.authority,
                self.config.local,
                Action::Observe,
                bytes,
            )?;
        }
        let timing = match (info.sent, info.received) {
            (Some(s), Some(r)) if s.epoch == r.epoch => r
                .milliseconds
                .checked_sub(s.milliseconds)
                .filter(|elapsed| *elapsed <= info.response)
                .map_or(Timing::Ambiguous, Timing::Within),
            _ => Timing::Ambiguous,
        };
        let evidence = op.completion.as_ref().map(|c| &c.output);
        Ok(state
            .pins
            .get(&key)
            .ok_or(Error::Unavailable)?
            .requester()
            .checks()
            .ok_or(Error::Unsupported)?
            .outcome(&op.metadata, evidence, expectations, timing))
    }
}
