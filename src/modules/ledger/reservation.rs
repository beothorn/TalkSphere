//! Durable operation-bound holds; successful reserve does not change ledger balances.
use super::{
    Error,
    authority::Action,
    contract::{Context, OperationKey},
    persistence::Persistence,
    state::{Ledger, State},
    terms::{Mode, Policy, Terms},
};
use crate::modules::identity::api::IdentityId;
use std::collections::BTreeMap;

/// Opaque operation association; holding this value never replaces coordinator authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    pub(super) local: IdentityId,
    pub(super) key: OperationKey,
}
/// Durable participant status; Reserved precedes the common prepared/decision protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Finite capacity held; no validated effect prepared yet.
    Reserved,
    /// Exact validated effect staged; only coordinator decision releases it.
    Prepared,
    /// Local effect committed once.
    Committed,
    /// Definitively aborted without successful settlement.
    Aborted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Resource {
    Owned(IdentityId),
    Issued(IdentityId),
    Issuance,
    Internal,
    Incoming(IdentityId),
    IssuedRoom(IdentityId),
}
#[derive(Clone)]
pub(super) struct Operation {
    pub terms: Terms,
    pub mode: Mode,
    pub resource: Resource,
    pub held: u64,
    pub status: Status,
    pub effect: Option<(u64, Vec<u8>)>,
    pub confirmed: bool,
    pub payment: super::payment::Payment,
}
impl<P: Persistence> Ledger<P> {
    /// Reserve one exact accepted provider-credit operation with durable retry association.
    /// # Errors
    /// Invalid roles/key, unsupported mode, authority, trust and capacity failures have no hold.
    pub fn reserve(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        terms: Terms,
        policy: Policy,
    ) -> Result<Reservation, Error> {
        tracing::trace!("Reserving finite Ledger settlement capacity");
        if key.requester != terms.requester {
            return Err(Error::Invalid);
        }
        self.change(context, Action::Coordinate, |s| {
            if let Some(op) = s.operations.get(&key) {
                if op.terms != terms {
                    return Err(Error::Conflict);
                }
                return Ok(Reservation {
                    local: self.local,
                    key,
                });
            }
            let trust = s
                .accounts
                .trust
                .get(&terms.requester)
                .copied()
                .unwrap_or_default();
            let mode = terms.assess(self.local, trust, policy)?;
            let resource = match mode {
                Mode::Provider => {
                    if self.local == terms.requester {
                        Resource::Owned(terms.provider)
                    } else {
                        Resource::Issued(terms.requester)
                    }
                }
                Mode::Requester => {
                    if self.local == terms.requester {
                        Resource::Issuance
                    } else {
                        Resource::Incoming(terms.requester)
                    }
                }
                Mode::Internal => Resource::Internal,
                Mode::ThirdParty => {
                    if self.local == terms.requester {
                        Resource::Owned(terms.issuer)
                    } else {
                        Resource::Incoming(terms.issuer)
                    }
                }
            };
            if mode == Mode::ThirdParty
                && s.operations.values().any(|o| {
                    o.terms.issuer == terms.issuer
                        && o.terms.requester == terms.requester
                        && o.terms.transfer == terms.transfer
                })
            {
                return Err(Error::Conflict);
            }
            let held = if mode == Mode::ThirdParty && self.local == terms.requester {
                terms
                    .maximum
                    .checked_add(terms.fee.ok_or(Error::Invalid)?)
                    .ok_or(Error::Limit)?
            } else {
                terms.maximum
            };
            if resource == Resource::Issuance
                && available(s, Resource::IssuedRoom(terms.provider))? < held
            {
                return Err(Error::Limit);
            }
            if s.operations.len() >= self.limits.operations {
                return Err(Error::Limit);
            }
            if available(s, resource)? < held {
                return Err(Error::Insufficient);
            }
            s.operations.insert(
                key,
                Operation {
                    terms,
                    mode,
                    resource,
                    held,
                    status: Status::Reserved,
                    effect: None,
                    confirmed: mode != Mode::ThirdParty,
                    payment: super::payment::Payment::Pending,
                },
            );
            Ok(Reservation {
                local: self.local,
                key,
            })
        })
    }
    /// Read remaining spendable owned or issued credits after every unresolved reservation.
    /// # Errors
    /// Requires local Read authority; unknown state and self-account requests fail.
    pub fn available(
        &self,
        context: &Context<'_>,
        peer: IdentityId,
        owned: bool,
    ) -> Result<u64, Error> {
        tracing::trace!("Reading spendable Ledger capacity");
        context.check(&self.authority, self.local, Action::Read, 0)?;
        if peer == self.local {
            return Err(Error::Invalid);
        }
        let i = self.inner.lock().map_err(|_| Error::Unknown)?;
        if i.uncertain {
            return Err(Error::Unknown);
        }
        available(
            &i.state,
            if owned {
                Resource::Owned(peer)
            } else {
                Resource::Issued(peer)
            },
        )
    }
}
// Compute exact remaining capacity under writer serialization; incoming holds protect uint64 headroom.
pub(super) fn available(s: &State, resource: Resource) -> Result<u64, Error> {
    tracing::trace!("Computing finite unreserved accounting capacity");
    let total = match resource {
        Resource::Owned(id) => s.accounts.owned.get(&id).copied().unwrap_or(0),
        Resource::Issued(id) => s.accounts.issued.get(&id).copied().unwrap_or(0),
        Resource::Issuance => s.issuance,
        Resource::Internal => s.internal,
        Resource::Incoming(id) => u64::MAX - s.accounts.owned.get(&id).copied().unwrap_or(0),
        Resource::IssuedRoom(id) => u64::MAX - s.accounts.issued.get(&id).copied().unwrap_or(0),
    };
    s.operations
        .values()
        .filter(|o| {
            (o.resource == resource || matches!(resource,Resource::IssuedRoom(id) if o.resource==Resource::Issuance && o.terms.provider==id))
                && (matches!(o.status, Status::Reserved | Status::Prepared)||(o.status==Status::Committed&&!o.confirmed))
        })
        .try_fold(total, |n, o| n.checked_sub(o.held).ok_or(Error::Invalid))
}

// Encode operation state explicitly rather than persisting process-local handles.
pub(super) fn encode(operations: &BTreeMap<OperationKey, Operation>) -> serde_json::Value {
    tracing::trace!("Encoding durable Ledger operation bindings");
    use serde_json::json;
    serde_json::Value::Array(
        operations
            .iter()
            .map(|(k, o)| {
                let t = &o.terms;
                let (r, id) = match o.resource {
                    Resource::Owned(id) => (0, id),
                    Resource::Issued(id) => (1, id),
                    Resource::Issuance => (2, IdentityId([0; 32])),
                    Resource::Internal => (3, IdentityId([0; 32])),
                    Resource::Incoming(id) => (4, id),
                    Resource::IssuedRoom(id) => (5, id),
                };
                json!([
                    k.requester.0,
                    k.connection,
                    k.stream,
                    t.requester.0,
                    t.provider.0,
                    t.issuer.0,
                    t.maximum,
                    t.fixed,
                    t.accepted,
                    t.transfer,
                    t.fee,
                    t.spending_bound,
                    o.mode as u8,
                    r,
                    id.0,
                    o.held,
                    o.status as u8,
                    o.effect.as_ref().map(|e| e.0),
                    o.effect.as_ref().map(|e| &e.1),
                    o.confirmed,
                    o.payment as u8
                ])
            })
            .collect(),
    )
}
// Validate every recovered operation identity and bound; canonical re-encoding rejects unknown extras.
pub(super) fn decode(
    v: &serde_json::Value,
    maximum: usize,
) -> Result<BTreeMap<OperationKey, Operation>, Error> {
    tracing::trace!("Decoding bounded Ledger operation journal");
    use super::state::number;
    let rows = v.as_array().ok_or(Error::Invalid)?;
    if rows.len() > maximum {
        return Err(Error::Limit);
    }
    let mut out = BTreeMap::new();
    for row in rows {
        let a = row
            .as_array()
            .filter(|a| a.len() == 21)
            .ok_or(Error::Invalid)?;
        let key = OperationKey {
            requester: IdentityId(array(&a[0])?),
            connection: array(&a[1])?,
            stream: u32::try_from(number(&a[2])?).map_err(|_| Error::Invalid)?,
        };
        let terms = Terms {
            requester: IdentityId(array(&a[3])?),
            provider: IdentityId(array(&a[4])?),
            issuer: IdentityId(array(&a[5])?),
            maximum: number(&a[6])?,
            fixed: a[7].as_bool().ok_or(Error::Invalid)?,
            accepted: byte_vec(&a[8])?,
            transfer: if a[9].is_null() {
                None
            } else {
                Some(array(&a[9])?)
            },
            fee: if a[10].is_null() {
                None
            } else {
                Some(number(&a[10])?)
            },
            spending_bound: number(&a[11])?,
        };
        let mode = match number(&a[12])? {
            0 => Mode::Provider,
            1 => Mode::Requester,
            2 => Mode::ThirdParty,
            3 => Mode::Internal,
            _ => return Err(Error::Invalid),
        };
        let id = IdentityId(array(&a[14])?);
        let resource = match number(&a[13])? {
            0 => Resource::Owned(id),
            1 => Resource::Issued(id),
            2 => Resource::Issuance,
            3 => Resource::Internal,
            4 => Resource::Incoming(id),
            5 => Resource::IssuedRoom(id),
            _ => return Err(Error::Invalid),
        };
        let status = match number(&a[16])? {
            0 => Status::Reserved,
            1 => Status::Prepared,
            2 => Status::Committed,
            3 => Status::Aborted,
            _ => return Err(Error::Invalid),
        };
        let effect = if a[17].is_null() && a[18].is_null() {
            None
        } else {
            Some((number(&a[17])?, byte_vec(&a[18])?))
        };
        let confirmed = a[19].as_bool().ok_or(Error::Invalid)?;
        let payment = match number(&a[20])? {
            0 => super::payment::Payment::Pending,
            1 => super::payment::Payment::Unknown,
            2 => super::payment::Payment::Unsettled,
            3 => super::payment::Payment::Settled,
            _ => return Err(Error::Invalid),
        };
        let held = number(&a[15])?;
        if key.requester != terms.requester || held == 0 || held < terms.maximum {
            return Err(Error::Invalid);
        }
        if out
            .insert(
                key,
                Operation {
                    terms,
                    mode,
                    resource,
                    held,
                    status,
                    effect,
                    confirmed,
                    payment,
                },
            )
            .is_some()
        {
            return Err(Error::Invalid);
        }
    }
    Ok(out)
}
// Decode fixed-width identifiers without truncation or lossy number conversion.
pub(super) fn array<const N: usize>(v: &serde_json::Value) -> Result<[u8; N], Error> {
    tracing::trace!("Decoding exact Ledger journal identifier");
    byte_vec(v)?.try_into().map_err(|_| Error::Invalid)
}
// Parse only arrays of exact bytes at the private state boundary.
pub(super) fn byte_vec(v: &serde_json::Value) -> Result<Vec<u8>, Error> {
    tracing::trace!("Decoding Ledger journal bytes");
    v.as_array()
        .ok_or(Error::Invalid)?
        .iter()
        .map(|n| u8::try_from(super::state::number(n)?).map_err(|_| Error::Invalid))
        .collect()
}

#[cfg(test)]
#[path = "tests/reservation.rs"]
mod tests;
