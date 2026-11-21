//! Exact family-owned metadata proposals; apply only after a durable coordinator commit decision.
use super::{
    contract::{Lease, State, Time},
    image::{Image, Limits},
    retention::Anchor,
};
use crate::modules::{identity::api::IdentityId, services::Error};

/// Immutable proposed metadata mutation; immutable bytes are staged separately before preparation.
#[derive(Clone)]
pub enum Effect {
    /// Read-only work retains a result but changes no lease description.
    NoChange,
    /// Create one independently accepted immutable lease.
    Create(Lease),
    /// Reserve the original lease fields before sampling time. Expiration holds the duration.
    CreateTemplate(Lease),
    /// Apply the exact lease and time observation durably bound to the Services decision.
    CreateAt {
        /// Finalized lease with the original fixed fields and exact protocol timestamps.
        lease: Lease,
        /// Logical owner time and the corresponding wall and elapsed observation at decision selection.
        anchor: Anchor,
    },
    /// Extend only the matching original expiration, preserving every other lease field.
    Renew {
        /// Exact active expiration on which this attempt was accepted.
        expected_expires_at: u64,
        /// Complete proposed lease after extension.
        lease: Lease,
    },
    /// End an active lease under its original requester authority.
    Remove {
        /// Exact original provider-issued lease identity.
        id: [u8; 32],
        /// Expiration observed at acceptance, preventing a concurrent renewal from being overwritten.
        expected_expires_at: u64,
    },
}
impl Effect {
    /// Encode an exact owner effect under finite image bounds; this is never a peer wire format.
    /// # Errors
    /// Invalid rejects inconsistent lease descriptions; Limit rejects encoded bounds.
    pub fn encode(&self, limits: Limits) -> Result<Vec<u8>, Error> {
        tracing::trace!("Encoding immutable Storage metadata proposal");
        let mut out = Vec::new();
        match self {
            Self::NoChange => out.push(0),
            Self::Create(lease) => {
                out.push(1);
                out.extend_from_slice(&lease_image(lease, limits)?);
            }
            Self::CreateTemplate(lease) => {
                if lease.accepted_at != 0 || lease.state != State::Active {
                    return Err(Error::Invalid);
                }
                out.push(4);
                out.extend_from_slice(&lease_image(lease, limits)?);
            }
            Self::CreateAt { lease, anchor } => {
                if lease.accepted_at != anchor.observation.unix_seconds {
                    return Err(Error::Invalid);
                }
                out.push(5);
                out.extend_from_slice(&encode_lease(lease, *anchor, limits)?);
            }
            Self::Renew {
                expected_expires_at,
                lease,
            } => {
                out.push(2);
                out.extend_from_slice(&expected_expires_at.to_be_bytes());
                out.extend_from_slice(&lease_image(lease, limits)?);
            }
            Self::Remove {
                id,
                expected_expires_at,
            } => {
                out.push(3);
                out.extend_from_slice(id);
                out.extend_from_slice(&expected_expires_at.to_be_bytes());
            }
        }
        if out.len() > limits.bytes {
            return Err(Error::Limit);
        }
        Ok(out)
    }
    /// Decode only a complete effect owned by the expected provider.
    /// # Errors
    /// Invalid rejects unknown/trailing/truncated formats; Denied rejects foreign owner; Limit rejects bounds.
    pub fn decode(bytes: &[u8], provider: IdentityId, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Decoding an original Storage metadata proposal");
        if bytes.len() > limits.bytes {
            return Err(Error::Limit);
        }
        match bytes.first() {
            Some(0) if bytes.len() == 1 => Ok(Self::NoChange),
            Some(1) => Ok(Self::Create(decode_lease(
                bytes.get(1..).ok_or(Error::Invalid)?,
                provider,
                limits,
            )?)),
            Some(2) => {
                let expected_expires_at = u64::from_be_bytes(
                    bytes
                        .get(1..9)
                        .ok_or(Error::Invalid)?
                        .try_into()
                        .map_err(|_| Error::Invalid)?,
                );
                Ok(Self::Renew {
                    expected_expires_at,
                    lease: decode_lease(bytes.get(9..).ok_or(Error::Invalid)?, provider, limits)?,
                })
            }
            Some(4) => {
                let lease = decode_lease(&bytes[1..], provider, limits)?;
                if lease.accepted_at != 0 || lease.state != State::Active {
                    return Err(Error::Invalid);
                }
                Ok(Self::CreateTemplate(lease))
            }
            Some(5) => {
                let image = Image::decode(&bytes[1..], provider, limits)?;
                let ([lease], []) = (image.leases.as_slice(), image.operations.as_slice()) else {
                    return Err(Error::Invalid);
                };
                if lease.accepted_at != image.anchor.observation.unix_seconds
                    || lease.state != State::Active
                {
                    return Err(Error::Invalid);
                }
                Ok(Self::CreateAt {
                    lease: lease.clone(),
                    anchor: image.anchor,
                })
            }
            Some(3) if bytes.len() == 41 => Ok(Self::Remove {
                id: bytes[1..33].try_into().map_err(|_| Error::Invalid)?,
                expected_expires_at: u64::from_be_bytes(
                    bytes[33..41].try_into().map_err(|_| Error::Invalid)?,
                ),
            }),
            _ => Err(Error::Invalid),
        }
    }
    /// Return the lease requiring exclusive mutation preparation, or None for read-only output.
    pub fn resource(&self) -> Option<[u8; 32]> {
        tracing::trace!("Identifying Storage metadata mutation scope");
        match self {
            Self::NoChange => None,
            Self::Create(l)
            | Self::CreateTemplate(l)
            | Self::CreateAt { lease: l, .. }
            | Self::Renew { lease: l, .. } => Some(l.id),
            Self::Remove { id, .. } => Some(*id),
        }
    }
    // Give recovery and quota accounting the original byte and identity binding.
    pub(super) fn created_lease(&self) -> Option<&Lease> {
        tracing::trace!("Reading Storage creation resource binding");
        match self {
            Self::Create(lease) | Self::CreateTemplate(lease) | Self::CreateAt { lease, .. } => {
                Some(lease)
            }
            _ => None,
        }
    }

    /// Apply the exact proposal to a private candidate image under original authenticated requester authority.
    /// Callers must persist the candidate only after the coordinator's immutable commit decision.
    /// # Errors
    /// Denied hides absent/foreign leases; Conflict rejects changed guards, inactive resources and reused IDs.
    pub fn apply(&self, image: &mut Image, requester: IdentityId) -> Result<(), Error> {
        tracing::trace!("Applying an original Storage metadata decision to a private candidate");
        match self {
            Self::NoChange => Ok(()),
            Self::CreateTemplate(_) => Err(Error::Conflict),
            Self::Create(lease) | Self::CreateAt { lease, .. } => {
                if lease.requester != requester || lease.provider != image.provider {
                    return Err(Error::Denied);
                }
                if lease.state != State::Active || image.leases.iter().any(|l| l.id == lease.id) {
                    return Err(Error::Conflict);
                }
                image.leases.push(lease.clone());
                Ok(())
            }
            Self::Renew {
                expected_expires_at,
                lease,
            } => {
                let index = authorized_index(image, requester, &lease.id)?;
                let old = &image.leases[index];
                if old.state != State::Active
                    || old.expires_at != *expected_expires_at
                    || lease.expires_at <= old.expires_at
                    || !unchanged(old, lease)
                {
                    return Err(Error::Conflict);
                }
                image.leases[index] = lease.clone();
                Ok(())
            }
            Self::Remove {
                id,
                expected_expires_at,
            } => {
                let index = authorized_index(image, requester, id)?;
                let old = &mut image.leases[index];
                if old.state != State::Active || old.expires_at != *expected_expires_at {
                    return Err(Error::Conflict);
                }
                old.state = State::Removed;
                Ok(())
            }
        }
    }
}
// Reuse only this owner's public bounded image format, not another module's private persistence schema.
fn lease_image(lease: &Lease, limits: Limits) -> Result<Vec<u8>, Error> {
    tracing::trace!("Encoding one exact proposed Storage lease");
    encode_lease(
        lease,
        Anchor::new(Time {
            unix_seconds: 0,
            elapsed_seconds: 0,
            boot: [0; 16],
        }),
        limits,
    )
}

// Encode the lease and its decision observation together using the owner's bounded image.
fn encode_lease(lease: &Lease, anchor: Anchor, limits: Limits) -> Result<Vec<u8>, Error> {
    tracing::trace!("Encoding Storage lease with original time observation");
    Image {
        provider: lease.provider,
        anchor,
        leases: vec![lease.clone()],
        operations: vec![],
    }
    .encode(limits)
}
// A proposal contains exactly one lease and no unrelated retained operations or mutable owner state.
fn decode_lease(bytes: &[u8], provider: IdentityId, limits: Limits) -> Result<Lease, Error> {
    tracing::trace!("Reading one bounded proposed Storage lease");
    let mut image = Image::decode(bytes, provider, limits)?;
    if image.leases.len() != 1 || !image.operations.is_empty() {
        return Err(Error::Invalid);
    }
    image.leases.pop().ok_or(Error::Invalid)
}
// Authorize before revealing active state, expected expiration or any other protected metadata.
fn authorized_index(image: &Image, requester: IdentityId, id: &[u8; 32]) -> Result<usize, Error> {
    tracing::trace!("Authorizing an original Storage mutation target");
    let index = image
        .leases
        .iter()
        .position(|l| l.id == *id)
        .ok_or(Error::Denied)?;
    super::lease::authorize(Some(&image.leases[index]), image.provider, requester, id)?;
    Ok(index)
}
// Renewal changes only expiration; identity, immutable content, original economics and acceptance remain fixed.
fn unchanged(old: &Lease, new: &Lease) -> bool {
    tracing::trace!("Checking Storage renewal preserves the original obligation");
    old.id == new.id
        && old.requester == new.requester
        && old.provider == new.provider
        && old.accepted_terms == new.accepted_terms
        && old.size_bytes == new.size_bytes
        && old.content_hash == new.content_hash
        && old.accepted_at == new.accepted_at
        && old.store_price == new.store_price
        && new.state == State::Active
}

#[cfg(test)]
#[path = "tests/effect.rs"]
mod tests;
