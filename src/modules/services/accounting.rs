//! Real Ledger participant adapter; Services never reads accounting private state.
use super::{
    Error,
    contract::{Context, OperationKey},
};
use crate::modules::ledger::{
    self,
    persistence::Persistence,
    reservation::Status,
    terms::{Policy, Terms},
};
use std::sync::Arc;

/// Narrow accounting participant contract supplied by trusted composition.
pub trait Accounting: Send + Sync {
    /// Reserve the original exact binding; repeated local requests cannot allocate a second hold.
    fn reserve(&self, context: &Context<'_>, key: OperationKey, terms: &Terms)
    -> Result<(), Error>;
    /// Prepare an exact final charge and retained effect reference idempotently.
    fn prepare(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        terms: &Terms,
        price: u64,
        effect: &[u8],
    ) -> Result<(), Error>;
    /// Commit only the original durably prepared effect after the coordinator decision.
    fn commit(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        terms: &Terms,
        price: u64,
        effect: &[u8],
    ) -> Result<(), Error>;
    /// Abort only after the explicit coordinator decision, preserving any prepared token association.
    fn abort(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        terms: &Terms,
        effect: Option<(u64, &[u8])>,
    ) -> Result<(), Error>;
    /// Observe original durable status without creating another attempt.
    fn status(&self, context: &Context<'_>, key: OperationKey) -> Result<Option<Status>, Error>;
    /// Reload an uncertain Ledger participant; never repeat the financial mutation.
    fn recover(&self, context: &Context<'_>) -> Result<(), Error>;
}
/// Protected direct adapter owns an explicitly issued Ledger coordinator grant and trusted acceptance policy.
pub struct Ledger<P: Persistence> {
    ledger: Arc<ledger::state::Ledger<P>>,
    grant: ledger::authority::Grant,
    policy: Policy,
}
impl<P: Persistence> Ledger<P> {
    /// Retain the real participant and composition-issued authority; no administrative grant is created here.
    pub fn new(
        ledger: Arc<ledger::state::Ledger<P>>,
        grant: ledger::authority::Grant,
        policy: Policy,
    ) -> Self {
        tracing::trace!("Binding real Ledger participant to Services accounting port");
        Self {
            ledger,
            grant,
            policy,
        }
    }
    // Pass original deadline/cancellation while checking Ledger's independent authority domain.
    fn context<'a>(&'a self, c: &Context<'_>) -> ledger::contract::Context<'a> {
        ledger::contract::Context {
            version: 1,
            grant: &self.grant,
            deadline: c.deadline,
            cancelled: c.cancelled,
        }
    }
}
// Preserve exact requester/connection/stream bytes across independent public module contracts.
fn key(k: OperationKey) -> ledger::contract::OperationKey {
    ledger::contract::OperationKey {
        requester: k.requester,
        connection: k.connection,
        stream: k.stream,
    }
}
impl<P: Persistence> Accounting for Ledger<P> {
    // The real Ledger serializes finite holds and retains exact retry associations.
    fn reserve(&self, c: &Context<'_>, k: OperationKey, t: &Terms) -> Result<(), Error> {
        tracing::trace!("Reserving through public Ledger contract");
        self.ledger
            .reserve(&self.context(c), key(k), t.clone(), self.policy)
            .map(|_| ())
            .map_err(Into::into)
    }
    // Reconstruct the owner-issued reservation through an exact retry, never a new invocation.
    fn prepare(
        &self,
        c: &Context<'_>,
        k: OperationKey,
        t: &Terms,
        p: u64,
        e: &[u8],
    ) -> Result<(), Error> {
        tracing::trace!("Preparing exact Services charge through Ledger");
        let c = self.context(c);
        let r = self.ledger.reserve(&c, key(k), t.clone(), self.policy)?;
        self.ledger
            .prepare(&c, &r, p, e.to_vec())
            .map(|_| ())
            .map_err(Into::into)
    }
    // The token is recovered by exact idempotent prepare; no opaque private field is serialized or forged.
    fn commit(
        &self,
        c: &Context<'_>,
        k: OperationKey,
        t: &Terms,
        p: u64,
        e: &[u8],
    ) -> Result<(), Error> {
        tracing::trace!("Delivering recorded Services commit to Ledger");
        let c = self.context(c);
        let r = self.ledger.reserve(&c, key(k), t.clone(), self.policy)?;
        let token = self.ledger.prepare(&c, &r, p, e.to_vec())?;
        self.ledger.commit(&c, &token).map_err(Into::into)
    }
    // Prepared aborts rebuild the exact token; explicit failure before preparation passes no token.
    fn abort(
        &self,
        c: &Context<'_>,
        k: OperationKey,
        t: &Terms,
        e: Option<(u64, &[u8])>,
    ) -> Result<(), Error> {
        tracing::trace!("Delivering recorded Services abort to Ledger");
        let c = self.context(c);
        if self.ledger.status(&c, key(k))?.is_none() {
            return Ok(());
        }
        let r = self.ledger.reserve(&c, key(k), t.clone(), self.policy)?;
        let token = e
            .map(|(p, e)| self.ledger.prepare(&c, &r, p, e.to_vec()))
            .transpose()?;
        self.ledger
            .abort(&c, &r, token.as_ref())
            .map_err(Into::into)
    }
    // Status reads do not create a new hold or assume absence after dependency loss.
    fn status(&self, c: &Context<'_>, k: OperationKey) -> Result<Option<Status>, Error> {
        tracing::trace!("Querying Ledger participant status for Services");
        self.ledger
            .status(&self.context(c), key(k))
            .map_err(Into::into)
    }
    // Ledger owns synchronized recovery of its possibly committed complete image.
    fn recover(&self, c: &Context<'_>) -> Result<(), Error> {
        tracing::trace!("Recovering uncertain Ledger participant for Services");
        self.ledger.recover(&self.context(c)).map_err(Into::into)
    }
}
impl From<ledger::Error> for Error {
    // Do not collapse possible commitment into an ordinary uncharged service failure.
    fn from(e: ledger::Error) -> Self {
        match e {
            ledger::Error::Invalid => Self::Invalid,
            ledger::Error::Unsupported => Self::Unsupported,
            ledger::Error::Denied => Self::Denied,
            ledger::Error::Limit => Self::Limit,
            ledger::Error::Cancelled => Self::Cancelled,
            ledger::Error::Conflict => Self::Conflict,
            ledger::Error::Insufficient => Self::Insufficient,
            ledger::Error::Storage => Self::Storage,
            ledger::Error::Unknown => Self::Unknown,
            ledger::Error::Unavailable => Self::Unavailable,
        }
    }
}
