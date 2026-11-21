//! Authorized renewal assessment and exact provider-size pricing without changing an existing lease.
use super::{
    contract::Time,
    image::Image,
    lease,
    operation::{self, Operation},
    pricing,
    terms::Terms,
};
use crate::modules::{
    board::offering::{Direction, Offering},
    services::{
        Error,
        handler::{Assessment, Peer},
        input::Input,
        pricing::Price,
        value::{Budget, Value},
    },
};

/// Trusted provider-owned assessment context; none of these identities or clock observations comes from input payloads.
pub struct Context<'a> {
    /// Authenticated original requester and local provider association.
    pub peer: &'a Peer,
    /// Currently selected compatible provider offering.
    pub offering: &'a Offering,
    /// Authoritative owner snapshot captured under serialized admission.
    pub image: &'a Image,
    /// Coherent provider clock observation.
    pub now: Time,
    /// Trusted elapsed seconds across boot change, or None during the original boot.
    pub reboot_elapsed: Option<u64>,
}
/// Provisional renewal assessment, not an extension, accepted execution or financial reservation.
pub struct Renewal {
    /// Known input metadata and exact retained size factor for public Services quote calculation.
    pub assessment: Assessment,
    /// Exact work charge under the current selected offering.
    pub price: u64,
    /// Proposed checked expiration in whole Unix seconds; execution must recheck the guard.
    pub expires_at: u64,
}

/// Authorize an active lease and assess exact renewal guards, limits and price before quote acceptance.
/// Small renewal fields must already be resolved; unresolved bodies fail explicitly without placeholder values.
/// # Errors
/// Denied collapses missing/foreign resources; Unavailable rejects inactive/expired leases;
/// Conflict rejects stale expiry, Invalid malformed terms/inputs/factors, Limit overflows/limits,
/// and Insufficient rejects work exceeding the caller's maximum accepted credits.
pub fn assess(
    context: Context<'_>,
    input: &Input,
    budget: Budget,
    maximum: u64,
) -> Result<Renewal, Error> {
    tracing::trace!("Assessing authorized Storage renewal without changing the lease");
    let Context {
        peer,
        offering,
        image,
        now,
        reboot_elapsed,
    } = context;
    if peer.local != image.provider
        || peer.key.requester != peer.remote
        || offering.direction != Direction::Provided
    {
        return Err(Error::Denied);
    }
    let input_bytes = u64::try_from(input.bounded(budget)?).map_err(|_| Error::Limit)?;
    let complete = input.complete(budget)?;
    if operation::input(&complete, budget)? != Operation::Renew {
        return Err(Error::Invalid);
    }
    let id = id(&complete)?;
    let current = lease::authorize(
        image.leases.iter().find(|l| l.id == id),
        peer.local,
        peer.remote,
        &id,
    )?;
    if !super::retention::active(image, current, now, reboot_elapsed)? {
        return Err(Error::Unavailable);
    }
    let expires_at = extension(&complete, current, Terms::parse(offering)?)?;
    let factors = Value::Object(vec![(
        "sizeBytes".into(),
        Value::Uint64(current.size_bytes),
    )]);
    let Price::Fixed(price) = price(offering, &complete, &factors, current, budget)? else {
        return Err(Error::Invalid);
    };
    if price > maximum {
        return Err(Error::Insufficient);
    }
    Ok(Renewal {
        assessment: Assessment {
            metadata: complete,
            factors,
            input_bytes,
            pin: None,
        },
        price,
        expires_at,
    })
}

/// Independently compute exact renewal pricing and require the sole provider factor to match retained requester metadata.
/// `lease` belongs to the caller's trusted catalog or provider owner; provider-asserted size alone is insufficient.
/// No byte object is allocated, even when retained size is uint64::MAX; current maximumBytes only restricts new stores.
/// # Errors
/// Invalid rejects malformed or extra factors/input; Conflict stale expected expiry;
/// Limit rejects current duration/total limits, addition or final price overflow.
pub fn price(
    offering: &Offering,
    input: &Value,
    factors: &Value,
    lease: &super::contract::Lease,
    budget: Budget,
) -> Result<Price, Error> {
    tracing::trace!(
        "Comparing Storage renewal size factor with independently retained lease metadata"
    );
    if offering.direction != Direction::Provided {
        return Err(Error::Denied);
    }
    if operation::input(input, budget)? != Operation::Renew || id(input)? != lease.id {
        return Err(Error::Invalid);
    }
    factors.bounded(Budget {
        depth: 1,
        nodes: 2,
        bytes: 9,
    })?;
    if !matches!(factors,Value::Object(f) if f.len()==1 && f[0].0=="sizeBytes" && f[0].1==Value::Uint64(lease.size_bytes))
        || lease.size_bytes == 0
    {
        return Err(Error::Invalid);
    }
    let terms = Terms::parse(offering)?;
    extension(input, lease, terms)?;
    Ok(Price::Fixed(pricing::usage(
        lease.size_bytes,
        number(input, "additionalSeconds")?,
        terms.renewal,
        terms.minimum_price,
    )?))
}

// Compare the original guard before checked additions; limit changes never shorten an existing lease.
fn extension(input: &Value, lease: &super::contract::Lease, terms: Terms) -> Result<u64, Error> {
    tracing::trace!("Checking original Storage renewal guard and current total retention bounds");
    let expected = number(input, "expectedExpiresAt")?;
    if expected != lease.expires_at {
        return Err(Error::Conflict);
    }
    let seconds = number(input, "additionalSeconds")?;
    if seconds < terms.minimum_seconds || seconds > terms.maximum_seconds {
        return Err(Error::Limit);
    }
    let expires = expected.checked_add(seconds).ok_or(Error::Limit)?;
    let latest = lease
        .accepted_at
        .checked_add(terms.maximum_total_seconds)
        .ok_or(Error::Limit)?;
    if expires > latest {
        return Err(Error::Limit);
    }
    Ok(expires)
}
// Read required bounded operation fields without interpreting extensions or allocating object bytes.
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    tracing::trace!("Reading original Storage renewal field");
    let Value::Object(f) = value else {
        return Err(Error::Invalid);
    };
    f.iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .ok_or(Error::Invalid)
}
// Preserve exact unsigned protocol seconds and reject lossy numeric substitutions.
pub(super) fn number(value: &Value, name: &str) -> Result<u64, Error> {
    tracing::trace!("Checking exact Storage renewal dimension");
    match field(value, name)? {
        Value::Uint64(n) => Ok(*n),
        _ => Err(Error::Invalid),
    }
}
// Decode only a fixed lease identifier, never a path, hash-based authority or provider-supplied ownership claim.
pub(super) fn id(value: &Value) -> Result<[u8; 32], Error> {
    tracing::trace!("Reading original Storage renewal identity");
    match field(value, "leaseId")? {
        Value::Bytes(b) => b.as_slice().try_into().map_err(|_| Error::Invalid),
        _ => Err(Error::Invalid),
    }
}
#[cfg(test)]
#[path = "tests/renewal.rs"]
mod tests;
