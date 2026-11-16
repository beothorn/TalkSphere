//! Atomic logical resource admission; durable owners retain/reconstruct holds across participant uncertainty.
use super::contract::Limits;
use crate::modules::services::{Error, contract::OperationKey};
use std::{collections::BTreeMap, sync::Mutex};

/// Logical resources charged to one original owner operation, including retained postcommit obligations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Staged, active and pinned logical bytes.
    pub bytes: u64,
    /// Retained lease slots, including terminal ID history when required.
    pub leases: usize,
    /// Pending operations; committed lease retention may use zero here.
    pub operations: usize,
    /// Retained output bytes independent of caller sessions.
    pub result_bytes: u64,
}
/// Process-local admission index reconstructed by its authoritative durable owner before new work.
pub struct Quota {
    limits: Limits,
    holds: Mutex<BTreeMap<OperationKey, Usage>>,
}
impl Quota {
    /// Create an empty finite admission index; recovery must restore all durable obligations before exposure.
    /// # Errors
    /// Invalid rejects unusable limits.
    pub fn new(limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Creating finite Storage admission index");
        Ok(Self {
            limits: limits.validate()?,
            holds: Mutex::new(BTreeMap::new()),
        })
    }
    /// Add one original-key hold without replacing an existing different binding.
    /// Identical repeats consume no additional capacity. This authorizes no financial effect.
    /// # Errors
    /// Conflict rejects changed reuse, Limit rejects exhausted capacity, Unavailable rejects a poisoned lock.
    pub fn reserve(&self, key: OperationKey, usage: Usage) -> Result<(), Error> {
        tracing::trace!("Reserving original-key Storage resource capacity");
        let mut holds = self.holds.lock().map_err(|_| Error::Unavailable)?;
        if let Some(existing) = holds.get(&key) {
            return if *existing == usage {
                Ok(())
            } else {
                Err(Error::Conflict)
            };
        }
        check(&holds, key, usage, self.limits)?;
        holds.insert(key, usage);
        Ok(())
    }
    /// Replace a held resource amount atomically after an authoritative durable transition.
    /// No capacity gap is exposed to concurrent requests. Pass zero usage only to remove the hold.
    /// # Errors
    /// Unavailable covers missing ownership/poisoned lock; Limit preserves the old hold on capacity failure.
    pub fn transition(&self, key: OperationKey, usage: Usage) -> Result<(), Error> {
        tracing::trace!("Updating retained Storage resource capacity");
        let mut holds = self.holds.lock().map_err(|_| Error::Unavailable)?;
        if !holds.contains_key(&key) {
            return Err(Error::Unavailable);
        }
        check(&holds, key, usage, self.limits)?;
        if usage == Usage::default() {
            holds.remove(&key);
        } else {
            holds.insert(key, usage);
        }
        Ok(())
    }
}
// Sum all logical obligations with checked arithmetic, replacing only the named original operation.
fn check(
    holds: &BTreeMap<OperationKey, Usage>,
    key: OperationKey,
    usage: Usage,
    limits: Limits,
) -> Result<(), Error> {
    tracing::trace!("Checking aggregate and per-owner Storage capacity");
    let mut total = usage;
    let mut owner = usage.operations;
    for (other, u) in holds {
        if *other == key {
            continue;
        }
        total.bytes = total.bytes.checked_add(u.bytes).ok_or(Error::Limit)?;
        total.leases = total.leases.checked_add(u.leases).ok_or(Error::Limit)?;
        total.operations = total
            .operations
            .checked_add(u.operations)
            .ok_or(Error::Limit)?;
        total.result_bytes = total
            .result_bytes
            .checked_add(u.result_bytes)
            .ok_or(Error::Limit)?;
        if other.requester == key.requester {
            owner = owner.checked_add(u.operations).ok_or(Error::Limit)?;
        }
    }
    if usage == Usage::default() && !holds.contains_key(&key) {
        return Err(Error::Invalid);
    }
    if total.bytes > limits.bytes
        || total.leases > limits.leases
        || total.operations > limits.operations
        || total.result_bytes > limits.result_bytes
        || owner > limits.per_owner
    {
        return Err(Error::Limit);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::identity::api::IdentityId;
    use std::{
        sync::{Arc, Barrier},
        thread,
    };
    // Trusted fixture keys distinguish same-owner operations from a second authenticated owner.
    fn key(owner: u8, stream: u32) -> OperationKey {
        OperationKey {
            requester: IdentityId([owner; 32]),
            connection: [1; 32],
            stream,
        }
    }
    // Each logical capacity is finite and repeated keys cannot multiply or silently alter holds.
    #[test]
    fn exact_holds_and_transitions() {
        let limits = Limits {
            bytes: 5,
            leases: 2,
            operations: 2,
            result_bytes: 5,
            per_owner: 1,
        };
        let q = Quota::new(limits).unwrap();
        let u = Usage {
            bytes: 3,
            leases: 1,
            operations: 1,
            result_bytes: 2,
        };
        assert_eq!(q.reserve(key(1, 1), u), Ok(()));
        assert_eq!(q.reserve(key(1, 1), u), Ok(()));
        assert_eq!(
            q.reserve(key(1, 1), Usage { bytes: 2, ..u }),
            Err(Error::Conflict)
        );
        assert_eq!(
            q.reserve(
                key(1, 2),
                Usage {
                    operations: 1,
                    ..Usage::default()
                }
            ),
            Err(Error::Limit)
        );
        assert_eq!(
            q.reserve(
                key(2, 1),
                Usage {
                    bytes: 3,
                    ..Usage::default()
                }
            ),
            Err(Error::Limit)
        );
        assert_eq!(
            q.reserve(
                key(2, 1),
                Usage {
                    leases: 2,
                    ..Usage::default()
                }
            ),
            Err(Error::Limit)
        );
        assert_eq!(
            q.reserve(
                key(2, 1),
                Usage {
                    operations: 2,
                    ..Usage::default()
                }
            ),
            Err(Error::Limit)
        );
        assert_eq!(
            q.reserve(
                key(2, 1),
                Usage {
                    result_bytes: 4,
                    ..Usage::default()
                }
            ),
            Err(Error::Limit)
        );
        assert_eq!(q.reserve(key(2, 1), Usage::default()), Err(Error::Invalid));
        assert_eq!(
            q.transition(key(1, 1), Usage { bytes: 6, ..u }),
            Err(Error::Limit)
        );
        assert_eq!(q.reserve(key(1, 1), u), Ok(()));
        assert_eq!(
            q.transition(key(1, 1), Usage { operations: 0, ..u }),
            Ok(())
        );
        assert_eq!(
            q.reserve(
                key(1, 2),
                Usage {
                    operations: 1,
                    ..Usage::default()
                }
            ),
            Ok(())
        );
        assert_eq!(q.transition(key(1, 1), Usage::default()), Ok(()));
        assert_eq!(
            q.transition(key(1, 1), Usage::default()),
            Err(Error::Unavailable)
        );
        assert_eq!(q.reserve(key(2, 1), u), Ok(()));
    }
    // Simultaneous attempts cannot both consume the same remaining byte capacity.
    #[test]
    fn concurrent_admission_is_atomic() {
        let q = Arc::new(
            Quota::new(Limits {
                bytes: 5,
                leases: 2,
                operations: 2,
                result_bytes: 5,
                per_owner: 1,
            })
            .unwrap(),
        );
        let gate = Arc::new(Barrier::new(3));
        let jobs: Vec<_> = (1..=2)
            .map(|owner| {
                let q = q.clone();
                let gate = gate.clone();
                thread::spawn(move || {
                    gate.wait();
                    q.reserve(
                        key(owner, 1),
                        Usage {
                            bytes: 4,
                            operations: 1,
                            ..Usage::default()
                        },
                    )
                })
            })
            .collect();
        gate.wait();
        let results: Vec<_> = jobs.into_iter().map(|j| j.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            results.iter().filter(|r| **r == Err(Error::Limit)).count(),
            1
        );
    }
    // Checked accumulation rejects overflow before replacing an acknowledged hold.
    #[test]
    fn aggregate_overflow_preserves_holds() {
        let q = Quota::new(Limits {
            bytes: u64::MAX,
            leases: 2,
            operations: 2,
            result_bytes: u64::MAX,
            per_owner: 1,
        })
        .unwrap();
        let huge = Usage {
            bytes: u64::MAX,
            result_bytes: u64::MAX,
            ..Usage::default()
        };
        assert_eq!(q.reserve(key(1, 1), huge), Ok(()));
        assert_eq!(
            q.reserve(
                key(2, 1),
                Usage {
                    bytes: 1,
                    ..Usage::default()
                }
            ),
            Err(Error::Limit)
        );
        assert_eq!(
            q.reserve(
                key(2, 1),
                Usage {
                    result_bytes: 1,
                    ..Usage::default()
                }
            ),
            Err(Error::Limit)
        );
        assert_eq!(q.reserve(key(1, 1), huge), Ok(()));
    }
}
