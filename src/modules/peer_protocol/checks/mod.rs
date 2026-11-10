//! Challenge-only remote checks bind exact quotes, durable reservations, evidence schemas and locally observed timing.
use super::{
    Error,
    admission::Gate,
    authority::{Action, Authority},
    contract::{Association, Context, Limits},
    quote::Entropy,
};
use crate::modules::{
    identity::api::IdentityId,
    services::{self, contract::OperationKey, registry::Registry, value::Budget},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, Weak},
    time::Instant,
};
pub mod clock;
pub mod evidence;
pub mod input;
mod provider;
mod requester;
mod state;
pub use provider::Issued;
pub use requester::{Candidate, Consented, Request};
pub use state::{Incoming, Outgoing};
/// Explicit real dependencies and finite local ownership for a remote check endpoint.
pub struct Config {
    /// Local authenticated node owner.
    pub local: IdentityId,
    /// Peer Protocol authority issuer.
    pub authority: Arc<Authority>,
    /// Separate Services authority issuer.
    pub services: Arc<services::authority::Authority>,
    /// Explicitly installed requester/provider check rules.
    pub registry: Arc<Registry>,
    /// Finite wire/value/quote budgets.
    pub limits: Limits,
    /// Independent read-only credit/trust policy.
    pub gate: Arc<dyn Gate>,
    /// Fresh random quote identifiers; never derived from challenge bytes.
    pub entropy: Arc<dyn Entropy>,
}
/// Local check wire owner; ordinary work cannot enter its zero-price entitlement path.
pub struct Owner {
    config: Config,
    domain: Arc<()>,
    slots: Mutex<BTreeMap<OperationKey, (Arc<()>, Instant)>>,
}
impl Owner {
    /// Bind configured local modules without installing remote-described code or reserving credits.
    /// # Errors
    /// Invalid bounds or a foreign Registry fail immediately.
    pub fn new(config: Config) -> Result<Self, Error> {
        tracing::trace!("Creating an explicitly configured remote-check owner");
        config.limits.validate()?;
        if config.registry.local_identity() != config.local {
            return Err(Error::Denied);
        }
        Ok(Self {
            config,
            domain: Arc::new(()),
            slots: Mutex::new(BTreeMap::new()),
        })
    }
    // Keep logical evidence/challenge budgets aligned with the endpoint's declared finite state.
    fn budget(&self) -> Budget {
        tracing::trace!("Reading finite check logical-value budget");
        Budget {
            depth: 31,
            nodes: self.config.limits.nodes,
            bytes: self.config.limits.state_bytes,
        }
    }
    // Both outgoing approval and incoming observation use the configured owner's separately scoped permission.
    fn check(&self, context: &Context<'_>, action: Action, bytes: usize) -> Result<(), Error> {
        tracing::trace!("Checking local check action authority");
        context.check(&self.config.authority, self.config.local, action, bytes)
    }
    // Opaque local capabilities cannot cross check owners even when public quote/stream fields match.
    fn owns(&self, weak: &Weak<()>) -> Result<(), Error> {
        tracing::trace!("Checking opaque check owner provenance");
        if !Arc::ptr_eq(&weak.upgrade().ok_or(Error::Denied)?, &self.domain) {
            return Err(Error::Denied);
        }
        Ok(())
    }
    // Only original authenticated associations can form a Services invocation key.
    fn association(&self, authorized: &services::authorization::Authorized) -> Association {
        tracing::trace!("Reading authorized original check association");
        let (requester, provider, _) = authorized.selection().roles();
        let key = authorized.key();
        Association {
            local: self.config.local,
            peer: if requester == self.config.local {
                provider
            } else {
                requester
            },
            operation: super::contract::OperationKey {
                requester: key.requester,
                connection: key.connection,
                stream: key.stream,
            },
        }
    }
}
