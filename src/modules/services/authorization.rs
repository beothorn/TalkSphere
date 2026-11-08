//! Invocation authority derives from selected application roles, never stream direction or peer hints.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Context, OperationKey},
    handler::Peer,
    registry::{Registered, Registry},
    value::Value,
};
use crate::modules::{board::selection::Selection, identity::api::IdentityId};

/// Authorized immutable selection and implementation pin for one accepted-attempt context.
pub struct Authorized {
    pub(crate) selection: Selection,
    pub(crate) implementation: Registered,
    pub(crate) peer: Peer,
    pub(crate) input: Option<Value>,
}
impl Authorized {
    /// Borrow exact selected terms without granting Ledger or provider execution authority.
    pub fn selection(&self) -> &Selection {
        tracing::trace!("Borrowing authorized immutable invocation selection");
        &self.selection
    }
    /// Read the trusted original operation key; identical input on another stream is a new attempt.
    pub fn key(&self) -> OperationKey {
        tracing::trace!("Reading authorized invocation attempt association");
        self.peer.key
    }
    /// Borrow explicitly supplied requester input; provider-side authorization does not invent it.
    pub fn input(&self) -> Option<&Value> {
        tracing::trace!("Borrowing explicitly authorized requester input");
        self.input.as_ref()
    }
    /// Borrow the exact pinned implementation associated with this selection.
    pub fn implementation(&self) -> &Registered {
        tracing::trace!("Borrowing authorized invocation implementation");
        &self.implementation
    }
}
/// Authorize an invocation with local input ownership and explicit requester/provider grants.
/// Peer context must come from authenticated composition, not decoded payload fields.
/// # Errors
/// Rejects foreign peers, mismatching attempt roles, absent local input, unsupported handlers and authority failures.
pub fn authorize(
    context: &Context<'_>,
    authority: &Authority,
    local: IdentityId,
    registry: &Registry,
    selection: Selection,
    peer: Peer,
    input: Option<Value>,
) -> Result<Authorized, Error> {
    tracing::trace!("Authorizing selected work using application requester and provider roles");
    let (requester, provider, _) = selection.roles();
    if peer.local != local
        || peer.key.requester != requester
        || peer.key.stream == 0
        || (local != requester && local != provider)
    {
        return Err(Error::Denied);
    }
    let opposite = if local == requester {
        provider
    } else {
        requester
    };
    if peer.remote != opposite {
        return Err(Error::Denied);
    }
    let requester_side = local == requester;
    let action = if requester_side {
        Action::Request
    } else {
        Action::Invoke
    };
    context.check(
        authority,
        local,
        action,
        selection.definition().document.bytes().len(),
    )?;
    if requester_side && input.is_none() {
        return Err(Error::Denied);
    }
    if let Some(value) = &input {
        let bytes = value.byte_len(super::value::Budget {
            depth: 31,
            nodes: 65536,
            bytes: 16_777_216,
        })?;
        context.check(
            authority,
            local,
            action,
            bytes
                .checked_add(selection.definition().document.bytes().len())
                .ok_or(Error::Limit)?,
        )?;
    }
    let o = selection.offering();
    let implementation = registry.select(&o.service, o.version, !requester_side)?;
    if !implementation
        .requester()
        .definition()
        .same_contract(selection.definition())
    {
        return Err(Error::Conflict);
    }
    implementation.requester().terms(o)?;
    Ok(Authorized {
        selection,
        implementation,
        peer,
        input,
    })
}
