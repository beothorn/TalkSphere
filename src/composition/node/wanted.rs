//! Wanted Storage contributions: the node pays clients in its own credits for holding its data.
//!
//! Callers: owner composition installs [`Work`] as the wanted publisher policy, `node-ctl wanted-add`
//! queues objects, and the runtime drains completions. The publisher (this node) is the requester: a
//! client that offers to fulfil the wanted terms stores one queued object, and the finite spending
//! bound is the policy's credits per object. The Ledger's issuance budget still bounds everything, and
//! the object, spending and provider are chosen only here, never by the peer's invitation.
use super::{
    catalog::FileCatalog,
    owners::{BUDGET, Owners},
    replicas::book::{Book, Replica},
};
use crate::{
    composition::policy::{Event, Kind, Wanted},
    modules::{
        board::selection::Selection,
        identity::{api::IdentityId, text},
        peer_protocol::{
            self as peer,
            engine::endpoint::requests::{Completion, Reply},
        },
        services::{self, value::Value},
        storage::requester::{self, catalog::remember},
    },
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, mpsc::Receiver},
    time::{Duration, Instant},
};

/// Most objects waiting for a contributor.
pub const MAXIMUM_BACKLOG: usize = 64;
/// How long a chosen object may stay in flight without a completion before it is considered lost.
const INFLIGHT_LIFETIME: Duration = Duration::from_secs(120);
/// Authority lifetime budget for the completion handler's local Services reads.
const OBSERVE_BYTES: usize = 16_777_216;

/// Why an object was not queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The object has no bytes.
    Empty,
    /// The object is larger than the wanted terms allow.
    TooLarge,
    /// The backlog is full.
    Full,
}

// Where a chosen object came from, so a result can be confirmed or returned to the right place.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// The operator's manual queue.
    Backlog,
    /// A hosted item that needs more replicas.
    Item([u8; 16]),
}

struct InFlight {
    provider: IdentityId,
    data: Vec<u8>,
    input: Value,
    origin: Origin,
    since: Instant,
}

#[derive(Default)]
struct State {
    backlog: VecDeque<Vec<u8>>,
    inflight: Vec<InFlight>,
}

/// The wanted work source and publisher policy.
pub struct Work {
    terms: Wanted,
    book: Arc<Book>,
    target: usize,
    state: Mutex<State>,
}

impl Work {
    /// A source for these wanted terms, serving hosted items from `book` until each has `target`
    /// replicas and then the manual queue, with nothing queued.
    pub fn new(terms: Wanted, book: Arc<Book>, target: usize) -> Self {
        tracing::trace!("Creating the wanted work source");
        Self {
            terms,
            book,
            target,
            state: Mutex::new(State::default()),
        }
    }

    /// Queue one object for the next contributor; returns how many are waiting.
    /// # Errors
    /// Empty or oversized objects and a full backlog are refused without queueing.
    pub fn enqueue(&self, data: Vec<u8>) -> Result<usize, Refusal> {
        tracing::trace!("Queueing an object for a wanted contribution");
        if data.is_empty() {
            return Err(Refusal::Empty);
        }
        if data.len() as u64 > self.terms.maximum_object_bytes {
            return Err(Refusal::TooLarge);
        }
        let mut state = self.lock();
        if state.backlog.len() >= MAXIMUM_BACKLOG {
            return Err(Refusal::Full);
        }
        state.backlog.push_back(data);
        Ok(state.backlog.len())
    }

    /// Objects waiting and objects currently with a contributor.
    pub fn counts(&self) -> (usize, usize) {
        tracing::trace!("Counting wanted work");
        let state = self.lock();
        (state.backlog.len(), state.inflight.len())
    }

    /// The wanted terms this source serves.
    pub fn terms(&self) -> &Wanted {
        tracing::trace!("Reading the wanted terms");
        &self.terms
    }

    // End the in-flight work of `provider`. A manual object that failed definitely returns to the front of
    // the backlog; a hosted item's reservation is always released (a confirmed replica is recorded
    // separately), so an unknown outcome never counts as a copy and nothing is stored twice by mistake.
    fn finish(&self, provider: IdentityId, requeue: bool) -> Option<(Value, Origin)> {
        let mut state = self.lock();
        let index = state.inflight.iter().position(|w| w.provider == provider)?;
        let work = state.inflight.remove(index);
        match work.origin {
            Origin::Backlog if requeue => state.backlog.push_front(work.data),
            Origin::Backlog => {}
            Origin::Item(_) => {}
        }
        Some((work.input, work.origin))
    }

    // A poisoned lock still holds consistent queues because every update is one push or remove.
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl peer::wanted::Policy for Work {
    // One object at a time per contributor; refuse when nothing is queued, the terms are not ours, or
    // the contributor already has work. The spending bound is exactly the published credits per object.
    fn choose(
        &self,
        selection: &Selection,
        provider: IdentityId,
    ) -> Result<peer::wanted::Work, peer::Error> {
        tracing::trace!("Choosing wanted work for a contributor");
        if selection.offering().id != super::offering::WANTED_ID {
            return Err(peer::Error::Denied);
        }
        let mut state = self.lock();
        let now = Instant::now();
        // Work that never completed is treated as lost: its outcome is unknown, so it is not re-queued,
        // and a hosted item's reservation is given back.
        let (lost, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut state.inflight)
            .into_iter()
            .partition(|w| now.duration_since(w.since) >= INFLIGHT_LIFETIME);
        state.inflight = kept;
        for work in lost {
            if let Origin::Item(id) = work.origin {
                self.book.release(id, work.provider);
            }
        }
        if state.inflight.iter().any(|w| w.provider == provider) {
            return Err(peer::Error::Limit);
        }
        // Hosted items that still need replicas come first; the manual queue serves the rest.
        let (data, origin) = match self.book.take_for(
            provider,
            self.target,
            self.terms.maximum_object_bytes,
            super::owners::unix_seconds(),
        ) {
            Some((id, sealed)) => (sealed, Origin::Item(id)),
            None => (
                state.backlog.pop_front().ok_or(peer::Error::Unavailable)?,
                Origin::Backlog,
            ),
        };
        let input = store_input(&data, self.terms.retention_seconds);
        state.inflight.push(InFlight {
            provider,
            data,
            input: input.clone(),
            origin,
            since: now,
        });
        Ok(peer::wanted::Work {
            input,
            spending: peer::requester::Spending {
                maximum: self.terms.credits_per_object,
                payment: Default::default(),
            },
        })
    }
}

// The typed Storage store input for one object.
fn store_input(data: &[u8], retention_seconds: u64) -> Value {
    let hash = crate::modules::cryptography::primitives::hash::sha256(data, data.len())
        .map(|h| h.to_vec())
        .unwrap_or_default();
    Value::Object(vec![
        ("operation".into(), Value::String("store".into())),
        ("sizeBytes".into(), Value::Uint64(data.len() as u64)),
        ("retentionSeconds".into(), Value::Uint64(retention_seconds)),
        ("contentHash".into(), Value::Bytes(hash)),
        ("data".into(), Value::Bytes(data.to_vec())),
    ])
}

/// Handle completions of wanted work until `stop` returns true: validate each committed store, catalog
/// the lease as one of this node's replicas and record the contribution. Definite failures return the
/// object to the backlog; interrupted work stays unknown and is never retried automatically.
pub fn drain(owners: &Owners, completions: &Receiver<Completion>, stop: impl Fn() -> bool) {
    tracing::trace!("Draining wanted work completions");
    let Some(work) = owners.wanted.as_ref() else {
        return;
    };
    while !stop() {
        let Ok(completion) = completions.recv_timeout(Duration::from_millis(250)) else {
            continue;
        };
        let provider = completion.scope.peer;
        match completion.result {
            Ok(Reply::Work(key)) => {
                let Some((input, origin)) = work.finish(provider, false) else {
                    tracing::warn!("A wanted contribution completed with no recorded input");
                    continue;
                };
                match validate_and_catalog(owners, key, &input, provider) {
                    Ok(lease) => {
                        if let Origin::Item(id) = origin {
                            let replica = Replica {
                                provider,
                                lease: lease.id,
                                expires_at: lease.expires_at,
                                confirmed_at: super::owners::unix_seconds(),
                                last_checked: None,
                            };
                            if owners.replicas.confirm(id, replica).is_err() {
                                tracing::warn!(
                                    "A validated replica could not be recorded in the book"
                                );
                            }
                        }
                        owners.observe(provider, Event::ContributionVerified);
                    }
                    Err(()) => {
                        tracing::warn!(
                            "A wanted contribution was committed but could not be validated"
                        );
                        if let Origin::Item(id) = origin {
                            owners.replicas.release(id, provider);
                        }
                        owners.observations.record(
                            provider,
                            Kind::Failed,
                            super::owners::unix_seconds(),
                        );
                    }
                }
            }
            Err(peer::Error::Unknown) => {
                tracing::warn!(
                    "A wanted contribution ended with an unknown outcome and will not be retried"
                );
                release_item(owners, work.finish(provider, false), provider);
            }
            Err(error) => {
                tracing::debug!(?error, "A wanted contribution failed before any effect");
                release_item(owners, work.finish(provider, true), provider);
            }
            Ok(_) => release_item(owners, work.finish(provider, false), provider),
        }
    }
}

// Give back the reservation of a hosted item whose contribution did not produce a confirmed replica.
fn release_item(owners: &Owners, finished: Option<(Value, Origin)>, provider: IdentityId) {
    if let Some((_, Origin::Item(id))) = finished {
        owners.replicas.release(id, provider);
    }
}

// Validate the committed result against the original input and keep the lease in the replica catalog.
fn validate_and_catalog(
    owners: &Owners,
    key: services::contract::OperationKey,
    input: &Value,
    provider: IdentityId,
) -> Result<crate::modules::storage::contract::Lease, ()> {
    tracing::trace!("Validating a wanted contribution");
    let grant = owners
        .services
        .grant(
            owners.local,
            services::authority::Action::Observe,
            OBSERVE_BYTES,
        )
        .map_err(|_| ())?;
    let context = services::contract::Context {
        version: 1,
        grant: &grant,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    };
    let lease = remember(
        &requester::catalog::Services {
            coordinator: &owners.runtime,
            context: &context,
        },
        owners.catalog.as_ref() as &FileCatalog,
        key,
        input,
        BUDGET,
        requester::store::Policy {
            observed_unix_seconds: super::owners::unix_seconds(),
            maximum_clock_difference_seconds: 60,
        },
    )
    .map_err(|_| ())?;
    owners
        .catalog
        .put(&lease, &text::format(provider))
        .map_err(|_| ())?;
    Ok(lease)
}

#[cfg(test)]
mod tests {
    use super::*;

    // An empty replica book in a fresh directory.
    fn book() -> Arc<Book> {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        Arc::new(
            Book::open(&std::env::temp_dir().join(format!(
                "ts-work-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
            .unwrap(),
        )
    }

    // Terms for tests: 7 credits per object of at most 8 bytes.
    fn terms() -> Wanted {
        Wanted {
            credits_per_object: 7,
            maximum_object_bytes: 8,
            retention_seconds: 60,
        }
    }

    // Objects are queued in order within bounds; empty, oversized and excess objects are refused.
    #[test]
    fn the_backlog_is_bounded_and_validated() {
        let work = Work::new(terms(), book(), 2);
        assert_eq!(work.enqueue(vec![]), Err(Refusal::Empty));
        assert_eq!(work.enqueue(vec![0; 9]), Err(Refusal::TooLarge));
        assert_eq!(work.enqueue(vec![1; 8]), Ok(1));
        for n in 1..MAXIMUM_BACKLOG {
            assert_eq!(work.enqueue(vec![2]), Ok(n + 1));
        }
        assert_eq!(work.enqueue(vec![3]), Err(Refusal::Full));
        assert_eq!(work.counts(), (MAXIMUM_BACKLOG, 0));
    }

    // A definite failure puts the object back first; success and unknown outcomes never do.
    #[test]
    fn only_definite_failures_requeue() {
        let work = Work::new(terms(), book(), 2);
        work.enqueue(vec![1]).unwrap();
        work.enqueue(vec![2]).unwrap();
        let state = |work: &Work| {
            let mut s = work.lock();
            let data = s.backlog.pop_front().unwrap();
            s.inflight.push(InFlight {
                provider: IdentityId([1; 32]),
                data: data.clone(),
                input: store_input(&data, 60),
                origin: Origin::Backlog,
                since: Instant::now(),
            });
        };
        state(&work);
        assert_eq!(work.counts(), (1, 1));
        assert!(work.finish(IdentityId([1; 32]), true).is_some());
        assert_eq!(work.lock().backlog.front(), Some(&vec![1]));
        state(&work);
        assert!(work.finish(IdentityId([1; 32]), false).is_some());
        assert_eq!(work.counts(), (1, 0));
        assert!(work.finish(IdentityId([1; 32]), false).is_none());
    }

    // The store input carries the exact size, retention, hash and bytes.
    #[test]
    fn store_input_is_exact() {
        let Value::Object(fields) = store_input(b"abc", 60) else {
            panic!("object expected")
        };
        let get = |name: &str| &fields.iter().find(|(n, _)| n == name).unwrap().1;
        assert_eq!(get("sizeBytes"), &Value::Uint64(3));
        assert_eq!(get("retentionSeconds"), &Value::Uint64(60));
        assert_eq!(get("data"), &Value::Bytes(b"abc".to_vec()));
        let Value::Bytes(hash) = get("contentHash") else {
            panic!("bytes expected")
        };
        assert_eq!(hash.len(), 32);
    }
}
