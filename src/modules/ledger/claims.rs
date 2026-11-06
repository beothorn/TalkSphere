//! Finite issuer claim commitments and lifetime creation/replay records; clear secrets are never stored.
use super::{
    Error,
    authority::Action,
    claim::Portable,
    contract::Context,
    persistence::Persistence,
    reservation::{Resource, available},
    state::{Ledger, State},
};
use crate::modules::{
    cryptography::{ports::Entropy, random::random_bytes},
    identity::api::IdentityId,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
/// Lifetime claim state; terminal entries cannot be forgotten while retaining the Ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimState {
    /// Finite issuance is reserved, no owner has redeemed it.
    Available,
    /// One original owner has redeemed; exact retry returns the stored result.
    Redeemed,
    /// Administrator invalidated before redemption and released reserved capacity.
    Invalidated,
}
#[derive(Clone)]
pub(super) struct ClaimRecord {
    pub creation: [u8; 32],
    pub hash: [u8; 32],
    pub amount: u64,
    pub restriction: Option<IdentityId>,
    pub state: ClaimState,
    pub owner: Option<IdentityId>,
}
impl<P: Persistence> Ledger<P> {
    /// Create one positive finite bearer claim under a durable administrative attempt key.
    /// # Errors
    /// Denied authority, repeated key/collision, entropy loss or insufficient capacity fails safely.
    /// Unknown commitment drops the secret; query the same creation key, never issue another claim blindly.
    pub fn create_claim(
        &self,
        context: &Context<'_>,
        creation: [u8; 32],
        amount: u64,
        restriction: Option<IdentityId>,
        entropy: &mut impl Entropy,
    ) -> Result<Portable, Error> {
        tracing::trace!("Creating finite credit claim with guarded bearer secret");
        context.check(&self.authority, self.local, Action::Administer, 64)?;
        if amount == 0 || restriction == Some(self.local) {
            return Err(Error::Invalid);
        }
        let id = *random_bytes::<32>(entropy).map_err(|_| Error::Storage)?;
        let secret = random_bytes::<32>(entropy).map_err(|_| Error::Storage)?;
        let hash: [u8; 32] = Sha256::digest(secret.as_ref()).into();
        self.change(context, Action::Administer, |s| {
            if s.claims.contains_key(&id) || s.claims.values().any(|c| c.creation == creation) {
                return Err(Error::Conflict);
            }
            if s.claims.len() >= self.limits.claims {
                return Err(Error::Limit);
            }
            if available(s, Resource::Issuance)? < amount {
                return Err(Error::Insufficient);
            }
            s.claims.insert(
                id,
                ClaimRecord {
                    creation,
                    hash,
                    amount,
                    restriction,
                    state: ClaimState::Available,
                    owner: None,
                },
            );
            Ok(())
        })?;
        Ok(Portable {
            issuer: self.local,
            id,
            secret,
            amount,
        })
    }
    /// Resolve an uncertain/lost private export by its durable creation key without reconstructing a secret.
    /// # Errors
    /// Requires Administer authority; uncertain state must recover before a definitive answer.
    pub fn claim_creation(
        &self,
        context: &Context<'_>,
        creation: [u8; 32],
    ) -> Result<Option<([u8; 32], ClaimState)>, Error> {
        tracing::trace!("Querying durable claim creation without returning bearer material");
        context.check(&self.authority, self.local, Action::Administer, 0)?;
        let i = self.inner.lock().map_err(|_| Error::Unknown)?;
        if i.uncertain {
            return Err(Error::Unknown);
        }
        Ok(i.state
            .claims
            .iter()
            .find(|(_, c)| c.creation == creation)
            .map(|(id, c)| (*id, c.state)))
    }
}
// Encode commitments and authoritative restrictions/results only; no clear secret exists in state.
pub(super) fn encode(s: &State) -> serde_json::Value {
    tracing::trace!("Encoding nonsecret lifetime claim commitments");
    serde_json::Value::Array(
        s.claims
            .iter()
            .map(|(id, c)| {
                serde_json::json!([
                    id,
                    c.creation,
                    c.hash,
                    c.amount,
                    c.restriction.map(|id| id.0),
                    c.state as u8,
                    c.owner.map(|id| id.0)
                ])
            })
            .collect(),
    )
}
// Reject malformed/colliding creation identities and impossible terminal ownership before recovery.
pub(super) fn decode(
    v: &serde_json::Value,
    local: IdentityId,
    maximum: usize,
) -> Result<BTreeMap<[u8; 32], ClaimRecord>, Error> {
    tracing::trace!("Validating recovered finite claim records");
    use super::{reservation::array, state::number};
    let rows = v.as_array().ok_or(Error::Invalid)?;
    if rows.len() > maximum {
        return Err(Error::Limit);
    }
    let mut out = BTreeMap::new();
    let mut creations = std::collections::BTreeSet::new();
    for row in rows {
        let row = row
            .as_array()
            .filter(|a| a.len() == 7)
            .ok_or(Error::Invalid)?;
        let id = array(&row[0])?;
        let creation = array(&row[1])?;
        let hash = array(&row[2])?;
        let amount = number(&row[3])?;
        let restriction = if row[4].is_null() {
            None
        } else {
            Some(IdentityId(array(&row[4])?))
        };
        let state = match number(&row[5])? {
            0 => ClaimState::Available,
            1 => ClaimState::Redeemed,
            2 => ClaimState::Invalidated,
            _ => return Err(Error::Invalid),
        };
        let owner = if row[6].is_null() {
            None
        } else {
            Some(IdentityId(array(&row[6])?))
        };
        if amount == 0
            || restriction == Some(local)
            || owner == Some(local)
            || (state == ClaimState::Redeemed) != owner.is_some()
            || !creations.insert(creation)
            || (owner.is_some() && restriction.is_some() && owner != restriction)
        {
            return Err(Error::Invalid);
        }
        if out
            .insert(
                id,
                ClaimRecord {
                    creation,
                    hash,
                    amount,
                    restriction,
                    state,
                    owner,
                },
            )
            .is_some()
        {
            return Err(Error::Invalid);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "tests/claims.rs"]
mod tests;
