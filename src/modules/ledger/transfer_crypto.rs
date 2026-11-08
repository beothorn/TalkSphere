//! Protected outgoing transfer signing and strict public verification through existing modules.
use super::{
    Error,
    authority::{Action, Authority},
    contract::Context,
    transfer::Order,
};
use crate::modules::{
    cryptography::{credential, ports::Entropy, primitives::signature},
    identity::{
        api::IdentityId,
        authority::{Capability, Sign},
        ports::KeyStore,
        provider::{Identity, IdentityHandle},
        record::CredentialRecord,
    },
};
/// Trusted scoped signing dependency; implementations never expose private keys.
pub trait TransferSigner {
    /// Sign exactly the canonical unsigned order after local spending authorization.
    /// # Errors
    /// Denied private authority or unavailable signing must not produce a signature.
    fn sign(&self, bytes: &[u8; 121]) -> Result<[u8; 64], Error>;
}
/// Adapter borrowing a loaded Identity handle and purpose-scoped signing capability.
pub struct IdentitySigner<'a, S: KeyStore<Record = CredentialRecord>, E: Entropy> {
    /// Protected Identity provider, owned by composition.
    pub identity: &'a Identity<S, E>,
    /// Loaded local handle without raw key access.
    pub handle: &'a IdentityHandle,
    /// Explicit byte-limited Sign capability for this handle.
    pub capability: &'a Capability<Sign>,
}
impl<S: KeyStore<Record = CredentialRecord>, E: Entropy> TransferSigner
    for IdentitySigner<'_, S, E>
{
    // Preserve Identity denial and dependency failures without exposing private material.
    fn sign(&self, bytes: &[u8; 121]) -> Result<[u8; 64], Error> {
        tracing::trace!("Signing transfer through protected Identity capability");
        self.identity
            .sign(self.handle, self.capability, bytes)
            .map_err(Error::from)
    }
}
/// Construct a fresh random transfer and sign its exact bytes under explicit spending authority.
/// # Errors
/// Invalid order/bounds, denied authority, entropy loss or failed signature/key binding rejects.
/// Explicit local outgoing payment request, separate from authenticated peer input.
#[derive(Clone, Copy)]
pub struct Create {
    /// Intended issuer of the credits.
    pub issuer: IdentityId,
    /// Intended recipient distinct from owner/issuer.
    pub new_owner: IdentityId,
    /// Exact authorized gross amount.
    pub amount: u64,
    /// Local public key that must match the signer and owner identity.
    pub public_key: [u8; 32],
}
pub fn create(
    context: &Context<'_>,
    authority: &Authority,
    local: IdentityId,
    request: Create,
    entropy: &mut impl Entropy,
    signer: &impl TransferSigner,
) -> Result<(Order, [u8; 64]), Error> {
    tracing::trace!("Creating explicitly authorized random transfer order");
    context.check(authority, local, Action::Spend, 121)?;
    let Create {
        issuer,
        new_owner,
        amount,
        public_key,
    } = request;
    Order {
        transfer_id: [0; 16],
        issuer,
        owner: local,
        new_owner,
        amount,
    }
    .encode()?;
    let mut id = [0; 16];
    entropy.fill(&mut id).map_err(Error::from)?;
    let order = Order {
        transfer_id: id,
        issuer,
        owner: local,
        new_owner,
        amount,
    };
    let bytes = order.encode()?;
    context.check(authority, local, Action::Spend, 121)?;
    let sig = signer.sign(&bytes)?;
    verify(order.owner, order.issuer, &order, &public_key, &sig)?;
    Ok((order, sig))
}
/// Verify exact owner authentication, local issuer and strict CR2SE signature/key profile.
/// # Errors
/// Wrong caller/issuer/key relation, malformed or invalid proof is denied before accounting.
pub fn verify(
    authenticated: IdentityId,
    local: IdentityId,
    order: &Order,
    key: &[u8],
    sig: &[u8],
) -> Result<(), Error> {
    tracing::trace!("Verifying authenticated transfer owner and canonical signature");
    let bytes = order.encode()?;
    if authenticated != order.owner
        || local != order.issuer
        || credential::identity_id(key).map_err(Error::from)? != order.owner.0
    {
        return Err(Error::Denied);
    }
    signature::verify(key, &bytes, sig, 121).map_err(Error::from)
}

#[cfg(test)]
#[path = "tests/transfer_crypto.rs"]
mod tests;
