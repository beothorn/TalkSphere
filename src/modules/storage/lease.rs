//! Lease authorization shared by provider operations; callers supply trusted authenticated identities.
use super::contract::{Lease, LeaseId};
use crate::modules::{identity::api::IdentityId, services::Error};

/// Authorize a candidate lease without revealing why an inaccessible resource was rejected.
/// `local` and `requester` originate from trusted invocation context, never payload identity fields.
/// The caller retains ownership of the returned immutable description.
/// # Errors
/// Denied covers missing ID, mismatched lookup, foreign requester and a different provider.
/// State/expiration checks follow authorization so they cannot expose foreign lease metadata.
pub fn authorize<'a>(
    candidate: Option<&'a Lease>,
    local: IdentityId,
    requester: IdentityId,
    id: &LeaseId,
) -> Result<&'a Lease, Error> {
    tracing::trace!("Authorizing Storage lease access from authenticated context");
    candidate
        .filter(|lease| lease.id == *id && lease.provider == local && lease.requester == requester)
        .ok_or(Error::Denied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{services::handler::Blob, storage::contract::State};
    // A guessed identifier, ciphertext hash or foreign provider never authorizes lease metadata disclosure.
    #[test]
    fn foreign_and_missing_look_identical() {
        let local = IdentityId([1; 32]);
        let owner = IdentityId([2; 32]);
        let other = IdentityId([3; 32]);
        let lease = Lease {
            id: [4; 32],
            requester: owner,
            provider: local,
            accepted_terms: Blob::new(vec![1], 1).unwrap(),
            size_bytes: 3,
            content_hash: [5; 32],
            accepted_at: 1000,
            expires_at: 1010,
            store_price: 17,
            state: State::Active,
        };
        assert!(std::ptr::eq(
            authorize(Some(&lease), local, owner, &lease.id).unwrap(),
            &lease
        ));
        for (candidate, provider, requester, id) in [
            (None, local, owner, lease.id),
            (Some(&lease), local, other, lease.id),
            (Some(&lease), other, owner, lease.id),
            (Some(&lease), local, owner, [5; 32]),
        ] {
            assert!(matches!(
                authorize(candidate, provider, requester, &id),
                Err(Error::Denied)
            ));
        }
        // Connection and stream identifiers are deliberately absent: a coordinated owner may reconnect.
        assert!(authorize(Some(&lease), local, owner, &lease.id).is_ok());
    }
}
