//! Placement lifecycle: expiration, the allowed transitions and per-placement serialization.
//!
//! Callers: the provider operations. Every change goes through [`Store::update`], which applies one closure to one
//! record under the store lock, so conflicting operations on a placement (recover versus remove, renew versus remove,
//! recover versus expiration) are serialized and exactly one definite ordering results. Expiration is applied lazily
//! with the provider clock: a pending placement whose expiry has been reached is expired the next time it is touched.
//! An operation uses the time at which it was accepted, so one accepted while the placement was active completes
//! under that time even if the clock passes the expiry while it runs.
use super::{
    error::Error,
    ids::PlacementId,
    operation::Receipt,
    placement::{Record, Removal},
    state::State,
    store::Store,
};

/// Expire a pending placement whose expiry was reached at `now`, then return the record. A recovered placement is kept as status
/// evidence until its expiry and then also reads as expired.
/// # Errors
/// `NotAvailable` for an unknown placement; `Storage` when recording the expiry fails.
pub fn touch(store: &Store, id: PlacementId, now: u64) -> Result<Record, Error> {
    tracing::trace!("Applying expiration to a placement");
    let record = store.record(id).ok_or(Error::NotAvailable)?;
    if record.state.is_active() && now >= record.expires_at {
        return store.update(id, &mut |r| {
            if r.state.is_active() && now >= r.expires_at {
                r.state = State::Expired;
            }
            Ok(())
        });
    }
    Ok(record)
}

/// Move a pending placement to recovered and record the receipt, in one step.
/// # Errors
/// `NotAvailable` unless the placement exists, is pending and is not expired at `now`; `Storage` on a failed write.
pub fn mark_recovered(
    store: &Store,
    id: PlacementId,
    now: u64,
    receipt: Receipt,
) -> Result<Record, Error> {
    tracing::trace!("Recording a recovery");
    store.update(id, &mut |r| {
        require_pending(r, now)?;
        move_to(r, State::Recovered)?;
        r.receipt = Some(receipt);
        Ok(())
    })
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
    tracing::trace!("Recording a removal");
    store.update(id, &mut |r| {
        require_pending(r, now)?;
        if r.expires_at != expected_expires_at {
            return Err(Error::Conflict);
        }
        move_to(r, State::Removed)?;
        r.removal = Some(removal);
        Ok(())
    })
}

/// Extend an active (pending or recovered) placement, guarded by the exact expected expiry. The new expiry is
/// `expected + additional_seconds`, which must not pass `ceiling_expires_at`, and the price is added to the payments, all with
/// overflow checks. The state does not change.
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
    tracing::trace!("Extending a placement");
    store.update(id, &mut |r| {
        if !r.active_at(now) {
            return Err(Error::NotAvailable);
        }
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

// A placement must be pending and unexpired to be recovered or removed.
fn require_pending(record: &Record, now: u64) -> Result<(), Error> {
    if record.state == State::Pending && now < record.expires_at {
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
    use crate::modules::identity::api::IdentityId;
    use crate::modules::messaging::{
        ids::{MessageId, PayloadHash, RecoveryId, RemovalId, Signature},
        metadata::Metadata,
        placement::Placement,
        state::Encoding,
        store::Limits,
    };
    use std::sync::Arc;

    const ID: PlacementId = PlacementId([1; 32]);

    // A store holding one pending placement accepted at 1000 and expiring at 1010 (price 17).
    fn store() -> Store {
        let store = Store::volatile(Limits {
            placements: 8,
            payload_bytes: 1000,
        });
        store
            .insert(&Placement {
                record: Record {
                    id: ID,
                    metadata: Metadata {
                        message_id: MessageId([1; 32]),
                        sender_id: IdentityId([1; 32]),
                        recipient_id: IdentityId([3; 32]),
                        sender_created_at: 1,
                        encoding: Encoding::Opaque,
                        payload_size: 3,
                        payload_hash: PayloadHash([4; 32]),
                    },
                    sender_public_key: [5; 32],
                    sender_signature: Signature([6; 64]),
                    accepted_at: 1000,
                    expires_at: 1010,
                    state: State::Pending,
                    payments: 17,
                    receipt: None,
                    removal: None,
                },
                payload: b"abc".to_vec(),
            })
            .unwrap();
        store
    }
    fn receipt() -> Receipt {
        Receipt {
            recovery_id: RecoveryId([7; 32]),
            recovered_at: 1005,
            recipient_public_key: [8; 32],
            recipient_signature: Signature([9; 64]),
            provider_signature: Signature([10; 64]),
        }
    }
    fn removal() -> Removal {
        Removal {
            removal_id: RemovalId([11; 32]),
            release_credits: 5,
            price: 2,
        }
    }

    // A pending placement is active until the instant of its expiry and expired from then on; touching records the expiry once.
    #[test]
    fn expiry_applies_exactly_at_the_boundary() {
        let s = store();
        assert_eq!(touch(&s, ID, 1009).unwrap().state, State::Pending);
        assert_eq!(touch(&s, ID, 1010).unwrap().state, State::Expired);
        assert_eq!(s.record(ID).unwrap().state, State::Expired);
        assert_eq!(touch(&s, ID, 5000).unwrap().state, State::Expired);
        assert_eq!(
            touch(&s, PlacementId([9; 32]), 1).err(),
            Some(Error::NotAvailable)
        );
    }

    // Recovery and removal work only on a pending, unexpired placement and each is final: nothing leaves a terminal state.
    #[test]
    fn recover_and_remove_are_exclusive_and_final() {
        let s = store();
        assert_eq!(
            mark_recovered(&s, ID, 1010, receipt()).err(),
            Some(Error::NotAvailable),
            "expired at the boundary"
        );
        let recovered = mark_recovered(&s, ID, 1005, receipt()).unwrap();
        assert_eq!(
            (recovered.state, recovered.receipt),
            (State::Recovered, Some(receipt()))
        );
        assert_eq!(
            mark_recovered(&s, ID, 1006, receipt()).err(),
            Some(Error::NotAvailable)
        );
        assert_eq!(
            mark_removed(&s, ID, 1006, 1010, removal()).err(),
            Some(Error::NotAvailable),
            "a recovered placement cannot be removed"
        );
        let s = store();
        let removed = mark_removed(&s, ID, 1005, 1010, removal()).unwrap();
        assert_eq!(
            (removed.state, removed.removal),
            (State::Removed, Some(removal()))
        );
        assert_eq!(
            mark_recovered(&s, ID, 1006, receipt()).err(),
            Some(Error::NotAvailable)
        );
        assert_eq!(
            mark_removed(&s, ID, 1006, 1010, removal()).err(),
            Some(Error::NotAvailable)
        );
        // A stale expiry expectation is a conflict and removes nothing.
        let s = store();
        assert_eq!(
            mark_removed(&s, ID, 1005, 1009, removal()).err(),
            Some(Error::Conflict)
        );
        assert_eq!(s.record(ID).unwrap().state, State::Pending);
    }

    // Renewal needs the exact expected expiry, an active placement, and stays within the ceiling; the state is unchanged.
    #[test]
    fn extension_is_guarded_and_bounded() {
        let s = store();
        let extended = extend(&s, ID, 1005, 1010, 4, 1200, 6).unwrap();
        assert_eq!(
            (extended.expires_at, extended.payments, extended.state),
            (1014, 23, State::Pending)
        );
        // The old expectation is now stale, so a retry cannot extend twice.
        assert_eq!(
            extend(&s, ID, 1006, 1010, 4, 1200, 6).err(),
            Some(Error::Conflict)
        );
        assert_eq!(s.record(ID).unwrap().expires_at, 1014);
        assert_eq!(
            extend(&s, ID, 1006, 1014, 187, 1200, 6).err(),
            Some(Error::Limit),
            "past the ceiling"
        );
        assert_eq!(
            extend(&s, ID, 1006, 1014, 186, 1200, 6).unwrap().expires_at,
            1200
        );
        assert_eq!(
            extend(&s, ID, 1200, 1200, 1, 1300, 1).err(),
            Some(Error::NotAvailable),
            "expired"
        );
        // A recovered placement can still be extended; its state does not change.
        let s = store();
        mark_recovered(&s, ID, 1005, receipt()).unwrap();
        assert_eq!(
            extend(&s, ID, 1006, 1010, 4, 1200, 6).unwrap().state,
            State::Recovered
        );
    }

    // Overflowing time or payments are limit failures that change nothing.
    #[test]
    fn extension_overflow_changes_nothing() {
        let s = store();
        assert_eq!(
            extend(&s, ID, 1005, 1010, u64::MAX, u64::MAX, 1).err(),
            Some(Error::Limit)
        );
        assert_eq!(
            extend(&s, ID, 1005, 1010, 1, 2000, u64::MAX).err(),
            Some(Error::Limit)
        );
        let r = s.record(ID).unwrap();
        assert_eq!((r.expires_at, r.payments), (1010, 17));
    }

    // MS-16 preview: many threads racing recover against remove on one pending placement produce exactly one winner, and the final
    // state matches the winner.
    #[test]
    fn racing_recover_and_remove_have_exactly_one_winner() {
        for _ in 0..20 {
            let s = Arc::new(store());
            let wins: Vec<bool> = std::thread::scope(|scope| {
                let handles: Vec<_> = (0..8)
                    .map(|n| {
                        let s = s.clone();
                        scope.spawn(move || {
                            if n % 2 == 0 {
                                mark_recovered(&s, ID, 1005, receipt()).is_ok()
                            } else {
                                mark_removed(&s, ID, 1005, 1010, removal()).is_ok()
                            }
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert_eq!(wins.iter().filter(|w| **w).count(), 1, "{wins:?}");
            let record = s.record(ID).unwrap();
            assert!(matches!(record.state, State::Recovered | State::Removed));
            assert_eq!(record.receipt.is_some(), record.state == State::Recovered);
            assert_eq!(record.removal.is_some(), record.state == State::Removed);
        }
    }

    // MS-16 preview: concurrent renewals from the same expected expiry produce exactly one winner and one definite expiry.
    #[test]
    fn racing_renewals_have_exactly_one_winner() {
        let s = Arc::new(store());
        let wins: Vec<bool> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..6)
                .map(|_| {
                    let s = s.clone();
                    scope.spawn(move || extend(&s, ID, 1005, 1010, 4, 1200, 6).is_ok())
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert_eq!(wins.iter().filter(|w| **w).count(), 1);
        assert_eq!(s.record(ID).unwrap().expires_at, 1014);
        assert_eq!(s.record(ID).unwrap().payments, 23);
    }

    // A recovery accepted just before the expiry completes under its acceptance time even if the clock has passed the expiry
    // by the time it is recorded elsewhere; a request accepted at the expiry fails.
    #[test]
    fn operations_use_their_acceptance_time() {
        let s = store();
        assert!(mark_recovered(&s, ID, 1009, receipt()).is_ok());
        let s = store();
        assert_eq!(
            mark_recovered(&s, ID, 1010, receipt()).err(),
            Some(Error::NotAvailable)
        );
    }
}
