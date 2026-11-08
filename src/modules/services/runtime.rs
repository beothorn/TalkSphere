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
        let records = operation::decode(&bytes, config.limits)?;
        let mut pins = BTreeMap::new();
        for (key, op) in &records {
            if op.terms.requester != config.local && op.terms.provider != config.local {
                return Err(Error::Denied);
            }
            let pin = registry.recover(context, &op.service, op.version)?;
            let document = crate::modules::board::document::Document::parse(
                &op.definition,
                Default::default(),
            )
            .map_err(|_| Error::Invalid)?;
            let definition = crate::modules::board::definition::Definition {
                document,
                support: super::schema::Support::Known,
            };
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
