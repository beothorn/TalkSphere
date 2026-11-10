//! Read-only economic preflight over explicit Ledger roles; final reservation remains atomic in Services/Ledger.
use super::{Error, contract::Context};
use crate::modules::{identity::api::IdentityId, ledger};
use std::sync::Arc;
/// Economic roles copied from a validated immutable Board selection.
#[derive(Debug, Clone, Copy)]
pub struct Roles {
    /// Explicit payer identity.
    pub requester: IdentityId,
    /// Work owner identity.
    pub provider: IdentityId,
    /// Accepted currency issuer.
    pub issuer: IdentityId,
}
/// Independently substitutable policy/capacity reads; no method reserves, charges or creates credit.
pub trait Gate: Send + Sync {
    /// Validate local participation, provider trust and explicitly configured issuer risk policy.
    fn check(&self, context: &Context<'_>, roles: Roles) -> Result<(), Error>;
    /// Check current finite capacity for exact maximum and fee; acceptance must still recheck atomically.
    fn capacity(
        &self,
        context: &Context<'_>,
        roles: Roles,
        maximum: u64,
        fee: u64,
    ) -> Result<(), Error>;
}
/// Real Ledger adapter retains a separate Read grant and immutable local provider acceptance policy.
pub struct LedgerGate<P: ledger::persistence::Persistence> {
    local: IdentityId,
    owner: Arc<ledger::state::Ledger<P>>,
    grant: ledger::authority::Grant,
    policy: ledger::terms::Policy,
}
impl<P: ledger::persistence::Persistence> LedgerGate<P> {
    /// Bind the exact configured Ledger owner without acquiring administrative authority.
    /// # Errors
    /// A mismatched local identity is Denied before use.
    pub fn new(
        local: IdentityId,
        owner: Arc<ledger::state::Ledger<P>>,
        grant: ledger::authority::Grant,
        policy: ledger::terms::Policy,
    ) -> Result<Self, Error> {
        tracing::trace!("Binding read-only Ledger economic preflight");
        if owner.local_identity() != local {
            return Err(Error::Denied);
        }
        Ok(Self {
            local,
            owner,
            grant,
            policy,
        })
    }
    // Carry the caller's deadline/cancellation into Ledger's independently scoped read boundary.
    fn context<'a>(&'a self, c: &Context<'_>) -> ledger::contract::Context<'a> {
        ledger::contract::Context {
            version: 1,
            grant: &self.grant,
            deadline: c.deadline,
            cancelled: c.cancelled,
        }
    }
}
impl<P: ledger::persistence::Persistence> Gate for LedgerGate<P> {
    // Provider trust and accepted issuer direction are local policy, never claims in peer JSON.
    fn check(&self, c: &Context<'_>, roles: Roles) -> Result<(), Error> {
        tracing::trace!("Checking provider trust and explicit credit policy");
        if self.local != roles.requester && self.local != roles.provider {
            return Err(Error::Denied);
        }
        let context = self.context(c);
        if self.local == roles.provider && roles.requester != roles.provider {
            let view = self.owner.view(&context, roles.requester)?;
            if view.trust.millionths() == 0 {
                return Err(Error::Denied);
            }
            if roles.issuer == roles.requester && !self.policy.requester_credits {
                return Err(Error::Unsupported);
            }
            if roles.issuer != roles.requester
                && roles.issuer != roles.provider
                && !self.policy.third_party
            {
                return Err(Error::Unsupported);
            }
        } else {
            self.owner.capacities(&context)?;
        }
        Ok(())
    }
    // Capacity is an observation, not a reservation guarantee; actual acceptance closes races under Ledger's lock.
    fn capacity(&self, c: &Context<'_>, roles: Roles, maximum: u64, fee: u64) -> Result<(), Error> {
        tracing::trace!("Checking exact finite quote capacity without reserving");
        self.check(c, roles)?;
        if maximum == 0 {
            return Err(Error::Invalid);
        }
        let total = maximum.checked_add(fee).ok_or(Error::Limit)?;
        let context = self.context(c);
        let available = if roles.requester == roles.provider {
            self.owner.capacities(&context)?.1
        } else if self.local == roles.requester {
            if roles.issuer == roles.requester {
                self.owner.capacities(&context)?.0
            } else {
                self.owner.available(&context, roles.issuer, true)?
            }
        } else if roles.issuer == self.local {
            self.owner.available(&context, roles.requester, false)?
        } else {
            u64::MAX - self.owner.view(&context, roles.issuer)?.owned
        };
        let required = if self.local == roles.requester {
            total
        } else {
            maximum
        };
        if available < required {
            return Err(Error::Insufficient);
        }
        Ok(())
    }
}
