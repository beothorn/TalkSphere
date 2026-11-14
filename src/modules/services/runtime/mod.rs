//! Durable local invocation coordinator for work and evidence checks over explicit module contracts.
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
    domain: Arc<()>,
    config: Config,
    journal: Journal<P>,
    accounting: Arc<dyn Accounting>,
    registry: Arc<Registry>,
    state: Mutex<State>,
}
impl<P: Persistence> Coordinator<P> {
    /// Read the configured local owner without granting operation, payload or accounting access.
    pub fn local_identity(&self) -> IdentityId {
        tracing::trace!("Reading Services coordinator local owner");
        self.config.local
    }

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
            if op.terms.provider == config.local {
                pin.provider().map_err(|_| Error::Unavailable)?;
            }
            let definition = if op.check.is_some() {
                op.check_invocation(config.limits, context.deadline)?
                    .definition
            } else {
                op.invocation(config.limits)?.definition
            };
            if !pin.requester().definition().same_contract(&definition) {
                return Err(Error::Unsupported);
            }
            let budget = Budget {
                depth: 31,
                nodes: config.limits.nodes,
                bytes: config.limits.state_bytes,
            };
            if op.check.is_some() {
                let rules = pin.requester().checks().ok_or(Error::Unsupported)?;
                rules.shape(&op.metadata)?;
                if let Some(input) = &op.input
                    && input != &op.metadata
                {
                    return Err(Error::Invalid);
                }
                if let Some(c) = &op.completion {
                    rules.evidence(&op.metadata, &c.output)?;
                }
            } else {
                if let Some(input) = &op.input {
                    crate::modules::services::validation::complete(
                        input,
                        &definition.document.value()["input"],
                        budget,
                        pin.requester(),
                    )?;
                }
                if let Some(c) = &op.completion {
                    crate::modules::services::validation::complete(
                        &c.output,
                        &definition.document.value()["output"],
                        budget,
                        pin.requester(),
                    )?;
                }
            }
            pins.insert(*key, pin);
        }
        Ok(Self {
            domain: Arc::new(()),
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
}

mod admission;
mod checks;
mod execution;
mod inspection;
mod payment;
mod recovery;
mod settlement;

mod replay;
