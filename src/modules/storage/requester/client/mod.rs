//! Installed Storage requester contracts connect Services quote/result validation to original local lease expectations.
mod checks;
mod contract;
mod input;
mod pricing;
mod result;
use crate::modules::{
    board::definition::Definition,
    identity::api::IdentityId,
    services::{Error, contract::OperationKey, handler::Blob, registry::Standards, value::Budget},
    storage::{
        contract::{Clock, Lease},
        standard::Catalogue,
    },
};
use std::sync::Arc;

/// Independently owned original attempt expectations; providers never receive requester encryption keys through this port.
pub trait Leases: Send + Sync {
    /// Return the original Storage lease snapshot bound to requester/provider/ID and this authenticated attempt key.
    /// Implementations retain the original snapshot across later catalog changes and must not resolve IDs by issuer alone.
    /// # Errors
    /// Missing/unowned expectations fail Denied; unavailable or uncertain local recovery remains an explicit error.
    fn lease(
        &self,
        key: OperationKey,
        provider: IdentityId,
        requester: IdentityId,
        id: [u8; 32],
    ) -> Result<Lease, Error>;
    /// Optionally return independently retained exact original range bytes for this attempt, never provider-supplied evidence.
    /// No expectation is distinct from a failed local lookup; a partial read without expected bytes verifies metadata only.
    /// # Errors
    /// Local dependency failures prevent verification rather than manufacturing expected bytes.
    fn range(
        &self,
        _key: OperationKey,
        _provider: IdentityId,
        _id: [u8; 32],
        _offset: u64,
        _length: u64,
    ) -> Result<Option<Blob>, Error> {
        tracing::trace!("Declaring absent independent Storage range expectations");
        Ok(None)
    }
}
/// Finite value policy and independently configured requester acceptance-clock tolerance.
#[derive(Clone, Copy)]
pub struct Policy {
    /// Maximum depth, logical nodes and aggregate byte content for each complete value.
    pub values: Budget,
    /// Maximum apparent difference between first observed store acceptance and the requester clock, in seconds.
    pub maximum_clock_difference_seconds: u64,
}
/// Real requester implementation; construction alone registers no handler, provider or Board offering.
pub struct Client {
    definition: Definition,
    leases: Arc<dyn Leases>,
    clock: Arc<dyn Clock>,
    policy: Policy,
}
impl Client {
    /// Construct installed requester code only after independent standard validation under explicit local policy.
    /// `leases` owns original attempt expectations and `clock` supplies requester observations at result validation.
    /// # Errors
    /// Invalid rejects unusable value bounds or altered schemas; Unsupported unknown contracts; Limit catalogue budgets.
    pub fn new(
        definition: Definition,
        catalogue: &Catalogue,
        leases: Arc<dyn Leases>,
        clock: Arc<dyn Clock>,
        policy: Policy,
    ) -> Result<Self, Error> {
        tracing::trace!(
            "Constructing Storage requester with independently validated standard and local expectations"
        );
        if policy.values.depth == 0
            || policy.values.depth > 31
            || policy.values.nodes == 0
            || policy.values.bytes == 0
        {
            return Err(Error::Invalid);
        }
        catalogue.validate(&definition)?;
        Ok(Self {
            definition,
            leases,
            clock,
            policy,
        })
    }
    // Caller and installed family budgets both apply; neither can expand the other's accepted resource policy.
    fn budget(&self, caller: Budget) -> Budget {
        tracing::trace!("Intersecting caller and installed Storage requester value budgets");
        Budget {
            depth: caller.depth.min(self.policy.values.depth),
            nodes: caller.nodes.min(self.policy.values.nodes),
            bytes: caller.bytes.min(self.policy.values.bytes),
        }
    }
    // Reject a substituted local expectation even if the dependency mistakenly returns metadata for another provider or owner.
    fn original(
        &self,
        key: OperationKey,
        provider: IdentityId,
        requester: IdentityId,
        id: [u8; 32],
    ) -> Result<crate::modules::storage::contract::Lease, Error> {
        tracing::trace!("Reading original provider-scoped Storage requester lease expectations");
        if key.requester != requester || key.stream == 0 {
            return Err(Error::Denied);
        }
        let lease = self.leases.lease(key, provider, requester, id)?;
        crate::modules::storage::lease::authorize(Some(&lease), provider, requester, &id)?;
        Ok(lease)
    }
}
#[cfg(test)]
#[path = "../../tests/client/mod.rs"]
mod tests;
