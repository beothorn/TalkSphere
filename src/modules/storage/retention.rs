//! Correction-resistant logical retention time; persistence and trusted reboot evidence belong to the owner.
use super::contract::{Lease, State, Time};
use crate::modules::services::Error;

/// Persistable clock anchor; elapsed seconds, rather than later UTC corrections, consume retention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    /// Logical Unix-second progress from the original UTC acceptance estimate.
    pub unix_seconds: u64,
    /// Original coherent platform observation for the last acknowledged anchor.
    pub observation: Time,
}
impl Anchor {
    /// Anchor an initial UTC estimate without granting any lease or changing time on existing leases.
    pub fn new(observation: Time) -> Self {
        tracing::trace!("Anchoring Storage retention to coherent platform time");
        Self {
            unix_seconds: observation.unix_seconds,
            observation,
        }
    }
    /// Compute a new immutable anchor from real elapsed time, independent of a UTC correction.
    /// On boot change, `reboot_elapsed` must be trusted elapsed seconds since the saved observation,
    /// supplied by composition's persistent time source; ordinary wall-clock subtraction is insufficient.
    /// # Errors
    /// Unavailable rejects an unmeasurable reboot interval. Invalid rejects backward boot-local time
    /// or unexpected reboot evidence within the same boot. Limit rejects logical time overflow.
    pub fn advance(self, now: Time, reboot_elapsed: Option<u64>) -> Result<Self, Error> {
        tracing::trace!("Advancing Storage retention by independently observed elapsed time");
        let elapsed = if now.boot == self.observation.boot {
            if reboot_elapsed.is_some() {
                return Err(Error::Invalid);
            }
            now.elapsed_seconds
                .checked_sub(self.observation.elapsed_seconds)
                .ok_or(Error::Invalid)?
        } else {
            reboot_elapsed.ok_or(Error::Unavailable)?
        };
        Ok(Self {
            unix_seconds: self.unix_seconds.checked_add(elapsed).ok_or(Error::Limit)?,
            observation: now,
        })
    }
}

/// Compute a store expiration without wrapping Unix-second arithmetic.
/// # Errors
/// Invalid rejects zero retention; Limit rejects an unrepresentable expiration.
pub fn expiration(accepted_at: u64, retention_seconds: u64) -> Result<u64, Error> {
    tracing::trace!("Calculating checked Storage expiration");
    if retention_seconds == 0 {
        return Err(Error::Invalid);
    }
    accepted_at
        .checked_add(retention_seconds)
        .ok_or(Error::Limit)
}

/// Mark an active lease expired at its deadline, leaving every terminal state irreversible.
/// Returns whether the owner must persist a transition before serving affected requests.
/// This changes only the candidate description; the authoritative owner must durably commit it.
pub fn refresh(lease: &mut Lease, logical_unix_seconds: u64) -> bool {
    tracing::trace!("Computing terminal Storage expiration transition");
    if lease.state == State::Active && logical_unix_seconds >= lease.expires_at {
        lease.state = State::Expired;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{identity::api::IdentityId, services::handler::Blob};
    // Wall-time jumps consume only true elapsed retention; unverified reboot intervals cannot expire data.
    #[test]
    fn correction_reboot_and_overflow() {
        let t = Time {
            unix_seconds: 1000,
            elapsed_seconds: 10,
            boot: [1; 16],
        };
        let a = Anchor::new(t);
        let forward = a
            .advance(
                Time {
                    unix_seconds: 9000,
                    elapsed_seconds: 11,
                    ..t
                },
                None,
            )
            .unwrap();
        assert_eq!(forward.unix_seconds, 1001);
        let backward = forward
            .advance(
                Time {
                    unix_seconds: 1,
                    elapsed_seconds: 12,
                    ..t
                },
                None,
            )
            .unwrap();
        assert_eq!(backward.unix_seconds, 1002);
        assert_eq!(
            a.advance(
                Time {
                    elapsed_seconds: 9,
                    ..t
                },
                None
            ),
            Err(Error::Invalid)
        );
        assert_eq!(a.advance(t, Some(1)), Err(Error::Invalid));
        let boot = Time {
            boot: [2; 16],
            elapsed_seconds: 0,
            ..t
        };
        assert_eq!(a.advance(boot, None), Err(Error::Unavailable));
        assert_eq!(a.advance(boot, Some(4)).unwrap().unix_seconds, 1004);
        assert_eq!(
            Anchor {
                unix_seconds: u64::MAX,
                ..a
            }
            .advance(
                Time {
                    elapsed_seconds: 11,
                    ..t
                },
                None
            ),
            Err(Error::Limit)
        );
        assert_eq!(expiration(1000, 10), Ok(1010));
        assert_eq!(expiration(1000, 0), Err(Error::Invalid));
        assert_eq!(expiration(u64::MAX, 1), Err(Error::Limit));
    }
    // Exact expiry is terminal; moving clocks backward never resurrects removed or expired leases.
    #[test]
    fn terminal_states_are_irreversible() {
        let mut lease = Lease {
            id: [1; 32],
            requester: IdentityId([2; 32]),
            provider: IdentityId([3; 32]),
            accepted_terms: Blob::new(vec![], 1).unwrap(),
            size_bytes: 1,
            content_hash: [4; 32],
            accepted_at: 1000,
            expires_at: 1010,
            store_price: 1,
            state: State::Active,
        };
        assert!(!refresh(&mut lease, 1009));
        assert!(refresh(&mut lease, 1010));
        assert!(!refresh(&mut lease, 1000));
        assert_eq!(lease.state, State::Expired);
        lease.state = State::Removed;
        assert!(!refresh(&mut lease, 2000));
        assert_eq!(lease.state, State::Removed);
        lease.state = State::Receiving;
        assert!(!refresh(&mut lease, 2000));
        assert_eq!(lease.state, State::Receiving);
    }
}
