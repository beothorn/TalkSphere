//! Durable local invocation coordinator; stage transitions retain exact accepted bindings before participant calls.
use super::{
    Error,
    accounting::Accounting,
    authority::{Action, Authority},
    codec,
    contract::{Context, Limits, OperationKey},
    journal::Journal,
    operation::{self, Operation},
    persistence::Persistence,
    quote::Accepted,
    registry::{Registered, Registry},
    value::{Budget, Value},
};
use crate::modules::{identity::api::IdentityId, ledger::terms::Terms};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

pub use super::operation::Phase;

/// Finite local runtime ownership and authority, supplied by composition.
pub struct Config {
    /// Identity whose journal and Ledger participant are coordinated.
    pub local: IdentityId,
    /// Services authority domain, independent of Ledger grants.
    pub authority: Arc<Authority>,
    /// Positive finite runtime capacities.
    pub limits: Limits,
}
/// Explicit immutable payment binding; third-party fees count inside the accepted spending ceiling.
#[derive(Clone, Copy, Default)]
pub struct Payment {
    /// Fresh third-party transfer identity, absent for two-party/internal work.
    pub transfer: Option<[u8; 16]>,
    /// Exact agreed issuer fee, absent outside third-party work.
    pub fee: Option<u64>,
}
/// Immutable coordinator decision; it does not assert remote receipt or payment success.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Deliver the exact already prepared successful local effects.
    Commit,
    /// Release uncommitted local resources without charging.
    Abort,
}
/// Complete original-provider result event, authenticated and assembled by the peer adapter.
pub struct RemoteResult {
    /// Complete typed logical output after peer decoding and explicit stream success/end.
    pub output: Value,
    /// Complete immutable original result bytes, independent of client session lifetime.
    pub bytes: super::handler::Blob,
    /// Exact reported final price, independently recalculated by Services.
    pub price: u64,
}
struct State {
    records: BTreeMap<OperationKey, Operation>,
    pins: BTreeMap<OperationKey, Registered>,
}
/// Single-writer recoverable coordinator over a real accounting participant and installed family ports.
pub struct Coordinator<P: Persistence> {
    config: Config,
    journal: Journal<P>,
    accounting: Arc<dyn Accounting>,
    registry: Arc<Registry>,
    state: Mutex<State>,
}
impl<P: Persistence> Coordinator<P> {
    /// Initialize absent state explicitly or reopen only complete versioned records and retained implementations.
    /// # Errors
    /// Missing/corrupt/foreign state, unsupported recovery implementations, authority and storage failures gate startup.
    pub fn open(
        config: Config,
        context: &Context<'_>,
        store: P,
        accounting: Arc<dyn Accounting>,
        registry: Arc<Registry>,
        initialize: bool,
    ) -> Result<Self, Error> {
        tracing::trace!("Opening durable Services invocation coordinator");
        config.limits.validate()?;
        if !registry.belongs_to(config.local, &config.authority) {
            return Err(Error::Denied);
        }
        let initial = operation::encode(&BTreeMap::new(), config.limits)?;
        let journal = Journal::open(
            context,
            config.local,
            config.authority.clone(),
            config.limits,
            store,
            initialize.then_some(initial.clone()),
        )?;
        let bytes = if initialize {
            initial
        } else {
            journal.read(context, Action::Recover)?
        };
        let records = operation::decode(&bytes, config.local, config.limits)?;
        let mut pins = BTreeMap::new();
        for (key, op) in &records {
            if op.terms.requester != config.local && op.terms.provider != config.local {
                return Err(Error::Denied);
            }
            let pin = registry.recover(context, &op.service, op.version)?;
            let invocation = op.invocation(config.limits)?;
            let definition = invocation.definition;
            if !pin.requester().definition().same_contract(&definition) {
                return Err(Error::Unsupported);
            }
            let budget = Budget {
                depth: 31,
                nodes: config.limits.nodes,
                bytes: config.limits.state_bytes,
            };
            if let Some(input) = &op.input {
                super::validation::complete(
                    input,
                    &definition.document.value()["input"],
                    budget,
                    pin.requester(),
                )?;
            }
            if let Some(completion) = &op.completion {
                super::validation::complete(
                    &completion.output,
                    &definition.document.value()["output"],
                    budget,
                    pin.requester(),
                )?;
            }
            pins.insert(*key, pin);
        }
        Ok(Self {
            config,
            journal,
            accounting,
            registry,
            state: Mutex::new(State { records, pins }),
        })
    }
    // Reject poisoned, uncertain or divergent images before effects, and recheck caller authority under serialization.
    fn check(&self, context: &Context<'_>, action: Action, state: &State) -> Result<(), Error> {
        context.check(&self.config.authority, self.config.local, action, 0)?;
        let bytes = self.journal.read(context, action)?;
        if bytes != operation::encode(&state.records, self.config.limits)? {
            return Err(Error::Unknown);
        }
        Ok(())
    }
    // Publish a new complete in-memory record set only after authoritative synchronized journal acknowledgement.
    fn save(&self, context: &Context<'_>, state: &mut State, op: Operation) -> Result<(), Error> {
        let old = operation::encode(&state.records, self.config.limits)?;
        let mut records = state.records.clone();
        records.insert(op.key, op);
        let bytes = operation::encode(&records, self.config.limits)?;
        self.journal.replace(context, &old, bytes)?;
        state.records = records;
        Ok(())
    }
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
                fixed: matches!(accepted.price, super::pricing::Price::Fixed(_)),
                accepted: accepted_bytes,
                transfer: payment.transfer,
                fee: payment.fee,
                spending_bound: accepted.maximum,
            },
            definition: selection.definition().document.bytes().to_vec(),
            metadata: accepted.assessed.assessment.metadata.clone(),
            pricing: accepted.price,
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
        self.accounting.reserve(context, key, &op.terms)?;
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
        let invocation = op.invocation(self.config.limits)?;
        let requester = state.pins.get(&key).ok_or(Error::Unavailable)?.requester();
        super::validation::complete(
            &input,
            &invocation.definition.document.value()["input"],
            budget,
            requester,
        )?;
        requester.validate_input(&op.metadata, &input, budget)?;
        op.input = Some(input);
        self.save(context, &mut state, op)
    }

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
        if op.terms.provider != self.config.local {
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
    fn retain(
        &self,
        context: &Context<'_>,
        state: &mut State,
        mut op: Operation,
        completion: super::handler::Completion,
    ) -> Result<(), Error> {
        tracing::trace!("Validating complete service result before durable retention");
        let pin = state.pins.get(&op.key).ok_or(Error::Unavailable)?;
        let invocation = op.invocation(self.config.limits)?;
        let budget = Budget {
            depth: 31,
            nodes: self.config.limits.nodes,
            bytes: self.config.limits.state_bytes,
        };
        super::validation::complete(
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
        if completion.effect.bytes().is_empty() {
            return Err(Error::Invalid);
        }
        if completion.result.bytes().len() > self.config.limits.state_bytes
            || completion.effect.bytes().len() > self.config.limits.state_bytes
        {
            return Err(Error::Limit);
        }
        let billed = if matches!(op.pricing, super::pricing::Price::Computation { .. }) {
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
    ) -> Result<(super::handler::Invocation, Value), Error> {
        tracing::trace!("Journaling requester dispatch before returning outbound invocation data");
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
        peer: super::handler::Peer,
        result: RemoteResult,
    ) -> Result<(), Error> {
        use sha2::{Digest, Sha256};
        tracing::trace!("Validating authenticated original-provider result event");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        let op = state.records.get(&peer.key).cloned().ok_or(Error::Denied)?;
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
        let effect = super::handler::Blob::new(Sha256::digest(result.bytes.bytes()).to_vec(), 32)?;
        self.retain(
            context,
            &mut state,
            op,
            super::handler::Completion {
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
    pub fn decline(&self, context: &Context<'_>, peer: super::handler::Peer) -> Result<(), Error> {
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
        if op.terms.provider == self.config.local && op.handler.is_none() {
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
        self.accounting.prepare(
            context,
            key,
            &op.terms,
            completion.price,
            &op.reference(self.config.limits)?,
        )?;
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
                    || (op.terms.provider == self.config.local && op.handler.is_none())
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
        let provider = if op.terms.provider == self.config.local {
            Some(state.pins.get(&key).ok_or(Error::Unavailable)?.provider()?)
        } else {
            None
        };
        if let Some(provider) = provider {
            let token = op.handler.as_ref().ok_or(Error::Invalid)?;
            match provider.status(key)? {
                super::handler::Status::Committed => {}
                super::handler::Status::Prepared(original) if &original == token => {
                    provider.commit(key, token)?
                }
                _ => return Err(Error::Unknown),
            }
        }
        match self.accounting.status(context, key)? {
            Some(crate::modules::ledger::reservation::Status::Committed) => {}
            Some(crate::modules::ledger::reservation::Status::Prepared) => self.accounting.commit(
                context,
                key,
                &op.terms,
                completion.price,
                &op.reference(self.config.limits)?,
            )?,
            _ => return Err(Error::Unknown),
        }
        if let Some(provider) = provider
            && !matches!(provider.status(key)?, super::handler::Status::Committed)
        {
            return Err(Error::Unknown);
        }
        if self.accounting.status(context, key)?
            != Some(crate::modules::ledger::reservation::Status::Committed)
        {
            return Err(Error::Unknown);
        }
        op.phase = Phase::Committed;
        self.save(context, &mut state, op)
    }

    /// Return a retained complete result only after all local effects are durably acknowledged.
    /// Payment completion and remote receipt remain independent of this local outcome.
    /// # Errors
    /// Pending/uncertain work returns Unknown; aborted work conflicts and authority remains required.
    pub fn result(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<super::handler::Completion, Error> {
        tracing::trace!("Reading locally committed retained Services result");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Observe, &state)?;
        let op = state.records.get(&key).ok_or(Error::Denied)?;
        if matches!(op.phase, Phase::Abort | Phase::Aborted) {
            return Err(Error::Conflict);
        }
        if op.phase != Phase::Committed {
            return Err(Error::Unknown);
        }
        op.completion.clone().ok_or(Error::Invalid)
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
        if op.terms.provider == self.config.local {
            let provider = state.pins.get(&key).ok_or(Error::Unavailable)?.provider()?;
            let token = if op.handler.is_some() {
                op.handler.clone()
            } else {
                match provider.status(key)? {
                    super::handler::Status::Prepared(token) => Some(token),
                    super::handler::Status::Committed => return Err(Error::Conflict),
                    _ => None,
                }
            };
            provider.abort(key, token.as_ref())?;
        }
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
        op.phase = Phase::Aborted;
        self.save(context, &mut state, op)
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

    /// Observe authoritative payment state only after local service work is committed.
    /// # Errors
    /// Unknown gates incomplete/missing accounting state; caller and dependency failures stay typed.
    pub fn payment(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<crate::modules::ledger::payment::Payment, Error> {
        tracing::trace!("Observing service-independent issuer payment state");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Observe, &state)?;
        if state.records.get(&key).ok_or(Error::Denied)?.phase != Phase::Committed {
            return Err(Error::Unknown);
        }
        self.accounting.payment(context, key)?.ok_or(Error::Unknown)
    }
    /// Describe an explicit original issuer transfer/query without performing network or signing work.
    /// # Errors
    /// Requires committed work, coordinator authority and supported third-party payment binding.
    pub fn payment_request(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<crate::modules::ledger::payment::PaymentRequest, Error> {
        tracing::trace!("Describing explicit issuer payment action for composition");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        if state.records.get(&key).ok_or(Error::Denied)?.phase != Phase::Committed {
            return Err(Error::Unknown);
        }
        self.accounting.payment_request(context, key)
    }
    /// Confirm payment only from the receiver-bound original-issuer evidence object.
    /// # Errors
    /// Wrong provenance, changed amount/fee/roles or missing commitment fail without executing work again.
    pub fn confirm_payment(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        receipt: &crate::modules::ledger::payment::Receipt,
    ) -> Result<(), Error> {
        tracing::trace!("Confirming original issuer evidence independently of service result");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        if state.records.get(&key).ok_or(Error::Denied)?.phase != Phase::Committed {
            return Err(Error::Unknown);
        }
        self.accounting.confirm_payment(context, key, receipt)
    }
    /// Retain refused or unknown payment without reversing successful local work or releasing its obligation.
    /// # Errors
    /// Requires committed third-party work, coordinator authority and an unresolved payment.
    pub fn payment_problem(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        unknown: bool,
    ) -> Result<(), Error> {
        tracing::trace!("Recording independent issuer refusal or delivery uncertainty");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        if state.records.get(&key).ok_or(Error::Denied)?.phase != Phase::Committed {
            return Err(Error::Unknown);
        }
        self.accounting.payment_problem(context, key, unknown)
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
    ) -> Result<super::handler::Check, Error> {
        tracing::trace!("Running requester-local verification without financial side effects");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Observe, &state)?;
        let op = state.records.get(&key).ok_or(Error::Denied)?;
        if op.phase != Phase::Committed {
            return Ok(super::handler::Check::Inconclusive);
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
            return Ok(super::handler::Check::Inconclusive);
        };
        let Some(completion) = &op.completion else {
            return Ok(super::handler::Check::Inconclusive);
        };
        let accepted =
            super::handler::Blob::new(op.terms.accepted.clone(), self.config.limits.state_bytes)?;
        match state
            .pins
            .get(&key)
            .ok_or(Error::Unavailable)?
            .requester()
            .verify(input, &accepted, &completion.output, expectations)
        {
            Err(Error::Unavailable | Error::Unknown) => Ok(super::handler::Check::Inconclusive),
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
