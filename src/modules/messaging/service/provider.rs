//! The Services provider adapter for Messaging.
//!
//! Services stages, prepares, finalizes and commits in separate calls, possibly across restarts. This adapter maps those calls onto
//! the provider engine's plan/prepare/commit split. `execute` validates the request, builds its effect and journals it as prepared
//! (the placement stays locked), and returns a *template* completion; `completion` rebuilds that template from the journal so no
//! work is repeated after a restart; `prepare` hands back the participant's token; stores and recoveries are finalized with the
//! coordinator's decision time and committed at exactly that time; everything else commits as is. Read-only operations keep their
//! encoded request in the journal so a lost staged result is produced again, never guessed.
//! Dependencies: the provider engine, the codec and the Services handler contracts.
use super::{outputs, project};
use crate::modules::{
    board::offering::{Direction, Offering},
    messaging::{
        codec::{self, Request, Response},
        effect::Effect,
        participant::Status as ParticipantStatus,
        provider::{Provider, RecoverPlan, RemovePlan},
    },
    services::{
        self, Error,
        checks::Invocation as CheckInvocation,
        contract::OperationKey,
        handler::{Assessment, Blob, Completion, Invocation, Peer, Status},
        input::Input,
        value::{Budget, Value},
    },
};
use std::sync::Arc;

/// The installed provider adapter. Recovery of the participant must complete before registration.
pub struct ServiceProvider {
    provider: Arc<Provider>,
    budget: Budget,
    maximum_bytes: usize,
}

impl ServiceProvider {
    /// Bind the engine to the value bounds Services uses for results.
    /// # Errors
    /// `Invalid` for a zero bound; `Unavailable` until the participant has completed recovery.
    pub fn new(
        provider: Arc<Provider>,
        budget: Budget,
        maximum_bytes: usize,
    ) -> Result<Self, Error> {
        tracing::trace!("Binding the Messaging provider adapter");
        if budget.depth == 0 || budget.nodes == 0 || budget.bytes == 0 || maximum_bytes == 0 {
            return Err(Error::Invalid);
        }
        // Nothing is served from a participant that has not completed recovery.
        if !provider.participant().is_ready() {
            return Err(Error::Unavailable);
        }
        Ok(Self {
            provider,
            budget,
            maximum_bytes,
        })
    }

    // The caller's budget intersected with the installed one, so neither can enlarge the other.
    fn bounded(&self, caller: Budget) -> Budget {
        Budget {
            depth: caller.depth.min(self.budget.depth),
            nodes: caller.nodes.min(self.budget.nodes),
            bytes: caller.bytes.min(self.budget.bytes),
        }
    }

    // Validate and describe one request without any effect.
    fn assess_value(
        &self,
        peer: Peer,
        offering: &Offering,
        input: &Input,
        budget: Budget,
    ) -> Result<Assessment, Error> {
        tracing::trace!("Assessing a Messaging request before quoting");
        let budget = self.bounded(budget);
        if peer.local != self.provider.participant().provider_id()
            || peer.remote != peer.key.requester
            || offering.direction != Direction::Provided
        {
            return Err(Error::Denied);
        }
        let declared = u64::try_from(input.bounded(budget)?).map_err(|_| Error::Limit)?;
        let caller = peer.remote;
        let empty = Value::Object(vec![]);
        if is_store(input) {
            let value = with_placeholders(input)?;
            let Request::Store(store) = codec::parse_request(&value).map_err(project::map)? else {
                return Err(Error::Invalid);
            };
            // The reservation taken here is released at once: the body has not arrived, and execute re-validates it in full.
            self.provider
                .assess_store(caller, store.header())
                .map_err(project::map)?;
            return Ok(Assessment {
                metadata: project::store_metadata(
                    store.message.metadata.payload_size,
                    store.retention_seconds,
                ),
                factors: empty,
                input_bytes: declared,
                pin: None,
            });
        }
        let value = input.complete(budget)?;
        let factors = match codec::parse_request(&value).map_err(project::map)? {
            Request::Store(_) => return Err(Error::Invalid),
            Request::Discover(request) => {
                if request.limit > self.provider.terms().maximum_discover_results {
                    return Err(Error::Invalid);
                }
                empty
            }
            Request::Status(request) => {
                self.provider
                    .status(caller, request)
                    .map_err(project::map)?;
                empty
            }
            Request::Recover(request) => {
                let size = self.size_of(request.placement_id)?;
                self.provider
                    .plan_recover(caller, request)
                    .map_err(project::map)?;
                project::factor("payloadSize", size)
            }
            Request::Renew(request) => {
                let size = self.size_of(request.placement_id)?;
                self.provider
                    .plan_renew(caller, request)
                    .map_err(project::map)?;
                project::factor("payloadSize", size)
            }
            Request::Remove(request) => {
                let credits = match self
                    .provider
                    .plan_remove(caller, request)
                    .map_err(project::map)?
                {
                    RemovePlan::Replay(done) => done.release_credits,
                    RemovePlan::Fresh(effect) => match *effect {
                        Effect::Remove {
                            release_credits, ..
                        } => release_credits,
                        _ => return Err(Error::Invalid),
                    },
                };
                project::factor("releaseCredits", credits)
            }
        };
        let input_bytes = u64::try_from(value.byte_len(budget)?).map_err(|_| Error::Limit)?;
        Ok(Assessment {
            metadata: value,
            factors,
            input_bytes,
            pin: None,
        })
    }

    // The payload size of a placement, which a recovery or renewal quote carries as its provider factor.
    fn size_of(
        &self,
        placement: crate::modules::messaging::ids::PlacementId,
    ) -> Result<u64, Error> {
        self.provider
            .participant()
            .store()
            .record(placement)
            .map(|record| record.metadata.payload_size)
            .ok_or(Error::Denied)
    }

    // The staged completion of a prepared effect: its template, or a freshly read result for a read-only operation.
    fn completion_of(&self, key: OperationKey, effect: &Effect) -> Result<Completion, Error> {
        if let Effect::Query { request } = effect {
            let value = services::codec::decode(request, self.budget, self.maximum_bytes)?;
            let (response, price) = match codec::parse_request(&value).map_err(project::map)? {
                Request::Discover(input) => {
                    let out = self
                        .provider
                        .discover(key.requester, input)
                        .map_err(project::map)?;
                    let price = out.price;
                    (Response::Discover(out), price)
                }
                Request::Status(input) => {
                    let out = self
                        .provider
                        .status(key.requester, input)
                        .map_err(project::map)?;
                    let price = out.price;
                    (Response::Status(out), price)
                }
                _ => return Err(Error::Invalid),
            };
            return outputs::completion(&response, price, effect, self.budget, self.maximum_bytes);
        }
        let (response, price) =
            outputs::template(effect, self.provider.participant().store()).map_err(project::map)?;
        outputs::completion(&response, price, effect, self.budget, self.maximum_bytes)
    }

    // The effect an operation key was prepared with, or the reason there is none.
    fn prepared(&self, key: OperationKey) -> Result<Effect, Error> {
        self.provider
            .participant()
            .prepared_effect(key)
            .ok_or(Error::Denied)
    }

    // Fill the decision-time fields of a template.
    fn fill(&self, effect: &Effect, template: &Completion, at: u64) -> Result<Completion, Error> {
        let mut response = codec::parse_response(&template.output).map_err(project::map)?;
        match (&mut response, effect) {
            (
                Response::Store(output),
                Effect::Store {
                    retention_seconds, ..
                },
            ) => {
                output.accepted_at = at;
                output.expires_at = at.checked_add(*retention_seconds).ok_or(Error::Invalid)?;
            }
            (Response::Recover(output), Effect::Recover { input, .. }) => {
                let record = self
                    .provider
                    .participant()
                    .store()
                    .record(input.placement_id)
                    .ok_or(Error::Denied)?;
                output.receipt = self
                    .provider
                    .participant()
                    .receipt_at(&record, input, at)
                    .map_err(project::map)?;
            }
            _ => return Err(Error::Invalid),
        }
        outputs::completion(
            &response,
            template.price,
            effect,
            self.budget,
            self.maximum_bytes,
        )
    }
}

// Inspect only the operation discriminator of an unresolved description.
fn is_store(input: &Input) -> bool {
    tracing::trace!("Checking the Messaging store discriminator");
    let is = |value: &Value| matches!(value, Value::String(name) if name == "store");
    match input {
        Input::Complete(Value::Object(fields)) => fields
            .iter()
            .any(|(name, value)| name == "operation" && is(value)),
        Input::Object(fields) => fields.iter().any(|(name, value)| {
            name == "operation" && matches!(value, Input::Complete(value) if is(value))
        }),
        _ => false,
    }
}

// A complete value for header parsing in which an unresolved body is a zero-filled placeholder of its declared length. The
// placeholder never leaves the assessment; the bounded input check has already limited the length.
fn with_placeholders(input: &Input) -> Result<Value, Error> {
    match input {
        Input::Complete(value) => Ok(value.clone()),
        Input::Object(fields) => Ok(Value::Object(
            fields
                .iter()
                .map(|(name, value)| Ok((name.clone(), with_placeholders(value)?)))
                .collect::<Result<_, Error>>()?,
        )),
        Input::Body {
            length,
            text: false,
        } => Ok(Value::Bytes(vec![
            0;
            usize::try_from(*length)
                .map_err(|_| Error::Limit)?
        ])),
        _ => Err(Error::Invalid),
    }
}

// The token bytes of a participant token blob.
fn token(blob: &Blob) -> Result<[u8; 32], Error> {
    blob.bytes().try_into().map_err(|_| Error::Invalid)
}

impl services::handler::Provider for ServiceProvider {
    // Complete values are assessed like unresolved ones.
    fn assess(&self, peer: Peer, offering: &Offering, input: &Value) -> Result<Assessment, Error> {
        tracing::trace!("Assessing a complete Messaging request");
        self.assess_value(peer, offering, &Input::Complete(input.clone()), self.budget)
    }

    // A store is assessed from its header without receiving the body.
    fn assess_metadata(
        &self,
        peer: Peer,
        offering: &Offering,
        input: &Input,
        budget: Budget,
    ) -> Result<Assessment, Error> {
        tracing::trace!("Assessing Messaging metadata before body admission");
        self.assess_value(peer, offering, input, budget)
    }

    // Validate the full request, journal its effect as prepared and return the staged template. A repeated call for the same
    // key returns the same staged result and never prepares a second effect.
    fn execute(&self, invocation: &Invocation, input: &Value) -> Result<Completion, Error> {
        tracing::trace!("Staging an accepted Messaging operation");
        let key = invocation.key;
        let caller = invocation.requester;
        let request = codec::parse_request(input).map_err(project::map)?;
        if let Some(prior) = self.provider.participant().prepared_effect(key) {
            return self.completion_of(key, &prior);
        }
        let participant = self.provider.participant();
        let effect = match request {
            Request::Store(store) => {
                let assessment = self
                    .provider
                    .assess_store(caller, store.header())
                    .map_err(project::map)?;
                self.provider
                    .plan_store(&assessment, store.message)
                    .map_err(project::map)?
            }
            Request::Recover(request) => match self
                .provider
                .plan_recover(caller, request)
                .map_err(project::map)?
            {
                RecoverPlan::Fresh(effect) => *effect,
                RecoverPlan::Replay { .. } => return Err(Error::Conflict),
            },
            Request::Renew(request) => self
                .provider
                .plan_renew(caller, request)
                .map_err(project::map)?,
            Request::Remove(request) => match self
                .provider
                .plan_remove(caller, request)
                .map_err(project::map)?
            {
                RemovePlan::Fresh(effect) => *effect,
                RemovePlan::Replay(_) => return Err(Error::Conflict),
            },
            Request::Discover(_) | Request::Status(_) => Effect::Query {
                request: services::codec::encode(input, self.budget, self.maximum_bytes)?,
            },
        };
        participant.prepare(key, &effect).map_err(project::map)?;
        self.completion_of(key, &effect)
    }

    // Rebuild the staged completion from the journal without doing the work again.
    fn completion(&self, key: OperationKey) -> Result<Completion, Error> {
        tracing::trace!("Recovering a staged Messaging completion");
        match self.provider.participant().prepared_effect(key) {
            Some(effect) => self.completion_of(key, &effect),
            None => Err(Error::Unknown),
        }
    }

    // The effect is already durably prepared; confirm it is exactly the staged one and return its token.
    fn prepare(&self, key: OperationKey, effect: &Blob) -> Result<Blob, Error> {
        tracing::trace!("Preparing the staged Messaging effect");
        let prepared = self.prepared(key)?;
        if prepared.encode() != effect.bytes() {
            return Err(Error::Conflict);
        }
        let token = self
            .provider
            .participant()
            .prepare(key, &prepared)
            .map_err(project::map)?;
        Blob::new(token.to_vec(), 32)
    }

    // Compare a result with the original local staged work: finalized results may differ from the template only in the decision-time
    // fields, and ordinary results must equal it exactly.
    fn validate_local_result(
        &self,
        invocation: &Invocation,
        input: &Value,
        result: &Completion,
    ) -> Result<bool, Error> {
        tracing::trace!("Validating a Messaging result against the original staged work");
        let effect = self.prepared(invocation.key)?;
        let original = self.completion_of(invocation.key, &effect)?;
        let expected = if self.requires_finalization(invocation, input) {
            let response = codec::parse_response(&result.output).map_err(project::map)?;
            let at = outputs::decision_time(&response).ok_or(Error::Invalid)?;
            self.fill(&effect, &original, at)?
        } else {
            original
        };
        if expected.output != result.output
            || expected.result != result.result
            || expected.price != result.price
            || expected.effect != result.effect
        {
            return Err(Error::Conflict);
        }
        Ok(true)
    }

    // Stores and recoveries carry decision-time fields.
    fn requires_finalization(&self, _: &Invocation, input: &Value) -> bool {
        tracing::trace!("Selecting Messaging decision-time finalization");
        matches!(project::operation(input), Ok("store" | "recover"))
    }

    // A template is valid when its effect belongs to this request and its output has exactly the promised shape. This reads no live
    // state, so it also holds after the placement's payload has been reclaimed and when the coordinator reopens its records.
    fn validate_template(
        &self,
        _invocation: &Invocation,
        input: &Value,
        template: &Completion,
    ) -> Result<(), Error> {
        tracing::trace!("Checking a Messaging template against its request");
        let request = codec::parse_request(input).map_err(project::map)?;
        let effect = Effect::decode(template.effect.bytes()).map_err(project::map)?;
        let response = codec::parse_response(&template.output).map_err(project::map)?;
        if !outputs::belongs(&request, &effect)
            || !outputs::has_template_shape(&effect, &response, template.price)
            || template.result.bytes()
                != services::codec::encode(&template.output, self.budget, self.maximum_bytes)?
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    // Sample the decision time and fill the template; no durable effect is applied.
    fn finalize(
        &self,
        key: OperationKey,
        token_blob: &Blob,
        template: &Completion,
    ) -> Result<Completion, Error> {
        tracing::trace!("Finalizing a Messaging result at the decision time");
        let effect = self.prepared(key)?;
        match self.provider.participant().status(key) {
            ParticipantStatus::Prepared(expected) if expected == token(token_blob)? => {}
            _ => return Err(Error::Denied),
        }
        let at = self.provider.now().map_err(project::map)?;
        self.fill(&effect, template, at)
    }

    // Apply the commit at the retained decision time and check that it produced exactly the finalized result.
    fn commit_finalized(
        &self,
        key: OperationKey,
        token_blob: &Blob,
        _template: &Completion,
        finalized: &Completion,
    ) -> Result<(), Error> {
        tracing::trace!("Committing a finalized Messaging result");
        let response = codec::parse_response(&finalized.output).map_err(project::map)?;
        let at = outputs::decision_time(&response).ok_or(Error::Invalid)?;
        let participant = self.provider.participant();
        let outcome = participant
            .commit_at(key, &token(token_blob)?, at)
            .map_err(project::map)?;
        let rebuilt = outputs::committed(&outcome, participant.store()).map_err(project::map)?;
        if codec::response_value(&rebuilt) != finalized.output {
            return Err(Error::Conflict);
        }
        Ok(())
    }

    // Operations without decision-time fields commit as prepared.
    fn commit(&self, key: OperationKey, token_blob: &Blob) -> Result<(), Error> {
        tracing::trace!("Committing a Messaging operation");
        self.provider
            .participant()
            .commit(key, &token(token_blob)?)
            .map(|_| ())
            .map_err(project::map)
    }

    // Release a prepared effect only on an explicit abort.
    fn abort(&self, key: OperationKey, token_blob: Option<&Blob>) -> Result<(), Error> {
        tracing::trace!("Aborting a Messaging operation");
        let token = token_blob.map(token).transpose()?;
        self.provider
            .participant()
            .abort(key, token.as_ref())
            .map_err(project::map)
    }

    // The durable participant state.
    fn status(&self, key: OperationKey) -> Result<Status, Error> {
        tracing::trace!("Reading the Messaging participant status");
        Ok(match self.provider.participant().status(key) {
            ParticipantStatus::Absent => Status::Absent,
            ParticipantStatus::Prepared(token) => Status::Prepared(Blob::new(token.to_vec(), 32)?),
            ParticipantStatus::Committed(_) => Status::Committed,
            ParticipantStatus::Aborted => Status::Aborted,
        })
    }

    // An exact retry of a recorded recovery or removal by the same authenticated caller may be answered from the original.
    fn allow_cached_retry(
        &self,
        original: &Invocation,
        peer: Peer,
        input: &Value,
    ) -> Result<bool, Error> {
        tracing::trace!("Deciding a Messaging cached retry");
        if original.requester != peer.remote {
            return Ok(false);
        }
        let store = self.provider.participant().store();
        Ok(match codec::parse_request(input).map_err(project::map)? {
            Request::Recover(request) => store.record(request.placement_id).is_some_and(|record| {
                record.metadata.recipient_id == peer.remote
                    && record
                        .receipt
                        .is_some_and(|receipt| receipt.recovery_id == request.recovery_id)
            }),
            Request::Remove(request) => store.record(request.placement_id).is_some_and(|record| {
                record.metadata.sender_id == peer.remote
                    && record
                        .removal
                        .is_some_and(|removal| removal.removal_id == request.removal_id)
            }),
            _ => false,
        })
    }

    // One authorized payload byte of evidence for a sender's paid check.
    fn evidence(&self, invocation: &CheckInvocation, input: &Value) -> Result<Value, Error> {
        tracing::trace!("Reading one authorized Messaging evidence byte");
        let challenge = codec::parse_check_input(input).map_err(project::map)?;
        let output = self
            .provider
            .check(invocation.requester, challenge)
            .map_err(project::map)?;
        Ok(codec::check_output_value(&output))
    }
}
