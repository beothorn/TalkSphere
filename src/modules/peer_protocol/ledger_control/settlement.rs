//! Third-party settlement consumes only normally completed independent-issuer observations, separate from successful work.
use crate::modules::{
    ledger::{
        self,
        payment::{PaymentRequest, Receipt},
        persistence::Persistence,
        state::Ledger,
    },
    peer_protocol::{
        Error,
        authority::{Action, Authority},
        contract::{Association, Context},
        ledger_control::query::{Request, Transfer},
    },
};
use std::sync::Arc;
/// Original local accounting owner plus independently scoped evidence/decision permissions.
pub struct Ports<'a, P: Persistence> {
    /// Real requester/provider Ledger, never the remote issuer's private state.
    pub owner: &'a Ledger<P>,
    /// Original local authority used to bind opaque issuer evidence.
    pub authority: Arc<ledger::authority::Authority>,
    /// Local Evidence permission for independently authenticated issuer data.
    pub evidence: &'a ledger::contract::Context<'a>,
    /// Separate local Coordinate permission for retained accepted payment.
    pub coordinate: &'a ledger::contract::Context<'a>,
}
/// Known issuer/refusal distinction does not alter the already committed service result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Issuer could not be reached or its commitment/reply remains ambiguous.
    Unreachable,
    /// Original issuer explicitly refused the agreed payment.
    Refused,
    /// Issuer no longer offers the exact preagreed fee.
    FeeChanged,
}
/// Read the exact retained submit/query action only for already committed third-party work.
/// # Errors
/// Wrong local scope, non-third-party/uncommitted/already settled work or uncertainty cannot create a settlement action.
pub fn next<P: Persistence>(
    context: &Context<'_>,
    authority: &Authority,
    original: Association,
    owner: &Ledger<P>,
    coordinate: &ledger::contract::Context<'_>,
) -> Result<PaymentRequest, Error> {
    tracing::trace!("Reading original committed work's explicit issuer settlement action");
    context.check(authority, owner.local_identity(), Action::Coordinate, 0)?;
    if original.local != owner.local_identity() {
        return Err(Error::Denied);
    }
    owner
        .payment_request(coordinate, key(original))
        .map_err(Error::from)
}
/// Create a provider's direct original-issuer query after the original invocation has ended normally.
/// # Errors
/// Forwarded/wrong issuer, reused invocation connection, requester-only submit actions and missing retained obligations are refused.
pub fn query<P: Persistence>(
    context: &Context<'_>,
    authority: Arc<Authority>,
    original: Association,
    issuer_stream: Association,
    owner: &Ledger<P>,
    coordinate: &ledger::contract::Context<'_>,
) -> Result<Request, Error> {
    tracing::trace!("Creating an independent original-issuer payment query");
    context.check(&authority, owner.local_identity(), Action::Request, 0)?;
    if original.local != owner.local_identity() {
        return Err(Error::Denied);
    }
    let action = owner.payment_request(coordinate, key(original))?;
    if action.submit {
        return Err(Error::Unsupported);
    }
    if issuer_stream.local != original.local
        || issuer_stream.peer != action.order.issuer
        || issuer_stream.operation.connection == original.operation.connection
    {
        return Err(Error::Denied);
    }
    Request::transfer(
        context,
        authority,
        issuer_stream,
        action.order.owner,
        action.order.transfer_id,
    )
}
/// Confirm original roles/ID/gross/fee/net once using only opaque normally completed authenticated issuer evidence.
/// # Errors
/// Wrong owner/issuer/connection, forwarded data, reused payment tuple or mismatched economics cannot settle or reverse work.
pub fn confirm<P: Persistence>(
    context: &Context<'_>,
    authority: &Authority,
    original: Association,
    observation: &Transfer,
    ports: Ports<'_, P>,
) -> Result<(), Error> {
    tracing::trace!("Confirming original third-party payment from independent issuer evidence");
    let local = ports.owner.local_identity();
    context.check(authority, local, Action::Coordinate, 121)?;
    let source = observation.association();
    if original.local != local
        || source.local != local
        || source.operation.connection == original.operation.connection
        || source.peer == original.peer
        || source.peer == local
    {
        return Err(Error::Denied);
    }
    let receipt = Receipt::authenticated(
        ports.evidence,
        ports.authority.clone(),
        local,
        observation.issuer(),
        observation.result().clone(),
    )?;
    ports
        .owner
        .confirm_payment(ports.coordinate, key(original), &receipt)
        .map_err(Error::from)
}
/// Record an explicit unsettled/unknown payment observation without undoing committed work or silently changing fees.
/// # Errors
/// Foreign/missing/uncommitted/already settled records and unavailable persistence remain typed failures.
pub fn problem<P: Persistence>(
    context: &Context<'_>,
    authority: &Authority,
    original: Association,
    owner: &Ledger<P>,
    coordinate: &ledger::contract::Context<'_>,
    problem: Problem,
) -> Result<(), Error> {
    tracing::trace!("Retaining third-party payment uncertainty separately from committed work");
    context.check(authority, owner.local_identity(), Action::Coordinate, 0)?;
    if original.local != owner.local_identity() {
        return Err(Error::Denied);
    }
    owner
        .payment_problem(coordinate, key(original), problem == Problem::Unreachable)
        .map_err(Error::from)
}
// Original service association remains the payment key; issuer control streams never replace it.
fn key(original: Association) -> ledger::contract::OperationKey {
    tracing::trace!("Preserving original invocation association for payment reconciliation");
    ledger::contract::OperationKey {
        requester: original.operation.requester,
        connection: original.operation.connection,
        stream: original.operation.stream,
    }
}
