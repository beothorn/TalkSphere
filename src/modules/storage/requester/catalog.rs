//! Catalog writes follow acknowledged requester coordination, never provisional peer output.
use super::store::{self, Policy};
use crate::modules::{
    identity::api::IdentityId,
    services::{
        Error,
        contract::{Context, OperationKey},
        handler::{Completion, Invocation},
        persistence::Persistence,
        runtime::Coordinator,
        value::{Budget, Value},
    },
    storage::contract::Lease,
};

/// Application-owned recovery catalog; Storage providers never receive this port or the requester's encryption keys.
pub trait Catalog {
    /// Retain one validated successful lease independently of client sessions and provider storage.
    /// Implementations should make exact repeated entries idempotent and reject conflicting provider/ID reuse.
    /// # Errors
    /// Catalog failure does not undo a service or charge that has already committed.
    fn retain(&self, lease: &Lease) -> Result<(), Error>;
}
/// Trusted local source of acknowledged requester results, supplied independently of peer input.
pub trait Committed {
    /// Identify the requester whose authorized coordinator result is being observed.
    fn local_identity(&self) -> IdentityId;
    /// Observe the original complete result only after all required local effects have committed.
    /// # Errors
    /// Uncommitted, unknown, missing or unauthorized results fail without catalog publication.
    fn completed(&self, key: OperationKey) -> Result<(Invocation, Completion), Error>;
}
/// Public Services adapter; Observe authority, phase and requester identity remain enforced at their owning boundaries.
pub struct Services<'a, P: Persistence> {
    /// Existing requester coordinator, including retained original results.
    pub coordinator: &'a Coordinator<P>,
    /// Authorized local Observe context, never a peer-provided token.
    pub context: &'a Context<'a>,
}
impl<P: Persistence> Committed for Services<'_, P> {
    // A complete result at a different local owner cannot authorize requester catalog publication.
    fn local_identity(&self) -> IdentityId {
        tracing::trace!("Reading requester catalog observation identity");
        self.coordinator.local_identity()
    }
    // Use the public committed observation port, without reading another owner's private journal encoding.
    fn completed(&self, key: OperationKey) -> Result<(Invocation, Completion), Error> {
        tracing::trace!("Observing acknowledged Storage result through Services");
        self.coordinator.completed(self.context, key)
    }
}

/// Validate and retain an original acknowledged store result in the requester application's catalog.
/// Retrying this local observation never reopens a peer invocation or authorizes another charge.
/// # Errors
/// Original correlation/identity mismatches fail Denied; uncommitted or malformed results never reach the catalog.
/// Catalog failure propagates without changing committed work, balances or trust.
pub fn remember(
    source: &dyn Committed,
    catalog: &dyn Catalog,
    key: OperationKey,
    input: &Value,
    budget: Budget,
    policy: Policy,
) -> Result<Lease, Error> {
    tracing::trace!(
        "Retaining only an acknowledged verified Storage lease in the requester catalog"
    );
    let (invocation, completion) = source.completed(key)?;
    if invocation.key != key || invocation.requester != source.local_identity() {
        return Err(Error::Denied);
    }
    let lease = store::validate(&invocation, input, &completion.output, budget, policy)?;
    if completion.price != lease.store_price {
        return Err(Error::Invalid);
    }
    catalog.retain(&lease)?;
    Ok(lease)
}
