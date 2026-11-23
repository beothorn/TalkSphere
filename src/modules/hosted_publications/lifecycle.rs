//! Placement lifecycle: expiration, the allowed transitions and per-placement serialization.
//!
//! Callers: the provider operations. Every change goes through [`Store::update`], which applies one closure to one record under the
//! store lock, so conflicting operations on a placement (remove versus renew, remove versus expiration) are serialized and exactly
//! one definite ordering results. Expiration is applied lazily with the host clock: a pending placement whose expiry has been
//! reached is expired the next time it is touched. An operation uses the time at which it was accepted, so one accepted while the
//! placement was active completes under that time even if the clock passes the expiry while it runs.
use super::{
    error::Error,
    ids::PlacementId,
    placement::{Record, Removal, State},
    store::Store,
};

/// Expire a pending placement whose expiry was reached at `now`, then return the record.
/// # Errors
/// `NotAvailable` for an unknown placement; `Storage` when recording the expiry fails.
pub fn touch(store: &Store, id: PlacementId, now: u64) -> Result<Record, Error> {
    tracing::trace!("Applying expiration to a hosted placement");
    let record = store.record(id).ok_or(Error::NotAvailable)?;
    if record.state.is_active() && now >= record.expires_at {
        return store.update(id, &mut |r| {
            if r.state.is_active() && now >= r.expires_at {
                move_to(r, State::Expired)?;
            }
            Ok(())
        });
    }
    Ok(record)
}

/// Move a pending placement to removed and record the removal result, once.
/// # Errors
/// `NotAvailable` unless pending and not expired at `now`; `Conflict` when `expected_expires_at` is not the current expiry.
pub fn mark_removed(
    store: &Store,
    id: PlacementId,
    now: u64,
    expected_expires_at: u64,
    removal: Removal,
) -> Result<Record, Error> {
    tracing::trace!("Recording a hosted removal");
    store.update(id, &mut |r| {
        require_active(r, now)?;
        if r.expires_at != expected_expires_at {
            return Err(Error::Conflict);
        }
        move_to(r, State::Removed)?;
        r.removal = Some(removal);
        Ok(())
    })
}

/// Extend a pending placement, guarded by the exact expected expiry. The new expiry is `expected + additional_seconds`, which must not
/// pass `ceiling_expires_at`, and the price is added to the payments, all with overflow checks. The state does not change.
/// # Errors
/// `NotAvailable` unless active at `now`; `Conflict` for a stale expectation; `Limit` for overflow or a passed ceiling.
pub fn extend(
    store: &Store,
    id: PlacementId,
    now: u64,
    expected_expires_at: u64,
    additional_seconds: u64,
    ceiling_expires_at: u64,
    price: u64,
) -> Result<Record, Error> {
    tracing::trace!("Extending a hosted placement");
    store.update(id, &mut |r| {
        require_active(r, now)?;
        if r.expires_at != expected_expires_at {
            return Err(Error::Conflict);
        }
        let expires = r
            .expires_at
            .checked_add(additional_seconds)
            .filter(|e| *e <= ceiling_expires_at)
            .ok_or(Error::Limit)?;
        r.payments = r.payments.checked_add(price).ok_or(Error::Limit)?;
        r.expires_at = expires;
        Ok(())
    })
}

// A placement must be pending and unexpired to be removed or renewed.
fn require_active(record: &Record, now: u64) -> Result<(), Error> {
    if record.active_at(now) {
        Ok(())
    } else {
        Err(Error::NotAvailable)
    }
}

// Change state through the allowed transitions only.
fn move_to(record: &mut Record, next: State) -> Result<(), Error> {
    if !record.state.may_become(next) {
        return Err(Error::Conflict);
    }
    record.state = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::hosted_publications::{
        ids::RemovalId,
        metadata::Kind,
        store::tests::{LIMITS, placement},
    };
    use std::sync::Arc;

    const ID: PlacementId = PlacementId([1; 32]);

    // A store holding one pending page placement accepted at 1000 and expiring at 1010.
    fn store() -> Store {
        let store = Store::volatile(LIMITS);
        store
            .insert(&placement(1, 3, Kind::Page, 1, 1000, 3), 1000)
            .unwrap();
        store
    }

    fn removal() -> Removal {
        Removal {
            removal_id: RemovalId([5; 32]),
            release_credits: 4,
            price: 2,
        }
    }

    // The expiry instant itself is expired: one second before it the placement is pending, at it the placement expires once.
    #[test]
    fn expiration_happens_at_the_expiry_instant() {
        let store = store();
        assert_eq!(touch(&store, ID, 1009).unwrap().state, State::Pending);
        assert_eq!(touch(&store, ID, 1010).unwrap().state, State::Expired);
        assert_eq!(touch(&store, ID, 5000).unwrap().state, State::Expired);
        assert_eq!(
            touch(&store, PlacementId([9; 32]), 1).err(),
            Some(Error::NotAvailable)
        );
        // Expired is final: nothing moves it.
        assert_eq!(
            mark_removed(&store, ID, 1011, 1010, removal()).err(),
            Some(Error::NotAvailable)
        );
        assert_eq!(
            extend(&store, ID, 1011, 1010, 5, 1200, 1).err(),
            Some(Error::NotAvailable)
        );
    }

    // Removal records its result exactly once, needs the exact expiry, and cannot happen at or after the expiry instant.
    #[test]
    fn removal_is_guarded_and_final() {
        let store = store();
        assert_eq!(
            mark_removed(&store, ID, 1005, 1011, removal()).err(),
            Some(Error::Conflict)
        );
        assert_eq!(
            mark_removed(&store, ID, 1010, 1010, removal()).err(),
            Some(Error::NotAvailable)
        );
        let record = mark_removed(&store, ID, 1005, 1010, removal()).unwrap();
        assert_eq!(
            (record.state, record.removal),
            (State::Removed, Some(removal()))
        );
        assert_eq!(
            mark_removed(&store, ID, 1006, 1010, removal()).err(),
            Some(Error::NotAvailable)
        );
        assert_eq!(
            extend(&store, ID, 1006, 1010, 5, 1200, 1).err(),
            Some(Error::NotAvailable)
        );
        // A removed placement never reads as expired later: terminal states do not move.
        assert_eq!(touch(&store, ID, 9999).unwrap().state, State::Removed);
    }

    // Renewal moves the expiry from the exact expectation, adds the price, and is refused when stale, past the ceiling or overflowing.
    #[test]
    fn extension_is_guarded_by_expectation_and_ceiling() {
        let store = store();
        assert_eq!(
            extend(&store, ID, 1005, 1000, 5, 1200, 1).err(),
            Some(Error::Conflict)
        );
        assert_eq!(
            extend(&store, ID, 1005, 1010, 191, 1200, 1).err(),
            Some(Error::Limit)
        );
        assert_eq!(
            extend(&store, ID, 1005, 1010, u64::MAX, u64::MAX, 1).err(),
            Some(Error::Limit)
        );
        let record = extend(&store, ID, 1005, 1010, 190, 1200, 15).unwrap();
        assert_eq!(
            (record.expires_at, record.payments, record.state),
            (1200, 32, State::Pending)
        );
        // The same expectation cannot win twice.
        assert_eq!(
            extend(&store, ID, 1006, 1010, 1, 1200, 1).err(),
            Some(Error::Conflict)
        );
        let huge = Store::volatile(LIMITS);
        let mut p = placement(2, 3, Kind::Page, 2, 1000, 1);
        p.record.payments = u64::MAX;
        huge.insert(&p, 1000).unwrap();
        assert_eq!(
            extend(&huge, p.record.id, 1001, 1010, 1, 1200, 1).err(),
            Some(Error::Limit)
        );
        assert_eq!(huge.record(p.record.id).unwrap().expires_at, 1010);
    }

    // HP-11 and HP-12 concurrency: eight threads racing one removal and renewals from one expectation produce exactly one winner and a
    // consistent final record, whatever the order.
    #[test]
    fn races_have_one_winner_and_a_consistent_record() {
        for _ in 0..20 {
            let store = Arc::new(store());
            let mut handles = Vec::new();
            for n in 0..8u8 {
                let store = Arc::clone(&store);
                handles.push(std::thread::spawn(move || {
                    if n % 2 == 0 {
                        mark_removed(&store, ID, 1005, 1010, removal()).is_ok()
                    } else {
                        extend(&store, ID, 1005, 1010, 5, 1200, 1).is_ok()
                    }
                }));
            }
            let winners = handles
                .into_iter()
                .filter_map(|h| h.join().ok())
                .filter(|w| *w)
                .count();
            assert_eq!(winners, 1);
            let record = store.record(ID).unwrap();
            match record.state {
                State::Removed => assert_eq!(record.expires_at, 1010),
                State::Pending => assert_eq!((record.expires_at, record.payments), (1015, 18)),
                State::Expired => panic!("expired before its expiry"),
            }
        }
    }
}
