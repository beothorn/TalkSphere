//! Replica maintenance: paid checks and renewals over the holders' live connections.
//!
//! The planner in [`crate::composition::node::replicas::maintain`] decides what is due; this file
//! performs it through the same peer operations an operator would run, so every check and renewal is
//! validated, journaled and charged exactly once under the policy's spending bound. An operation that
//! cannot be completed (the holder is offline, out of credits, or the result is inconclusive) is
//! remembered and retried only after the policy's retry delay, and never changes the replica count:
//! only a conclusive failed check or an ended retention removes a confirmed copy.
use super::{Handler, hex, now};
use crate::{
    composition::node::{
        control::protocol::Request,
        replicas::maintain::{Action, Attempts, MAXIMUM_PAID_ACTIONS, plan},
    },
    modules::{cryptography::random::SystemEntropy, storage::requester::sampling},
};
use serde_json::{Map, json};
use std::time::{Duration, Instant};

impl Handler {
    /// Perform whatever replica maintenance is due now, once.
    pub fn maintain_once(&self, attempts: &mut Attempts) {
        tracing::trace!("Running one replica maintenance pass");
        let items = self.owners.replicas.list();
        if items.is_empty() {
            return;
        }
        let policy = self.owners.policy.redundancy.clone();
        let retry = Duration::from_secs(policy.retry_seconds());
        let started = Instant::now();
        let actions = plan(
            &items,
            now(),
            policy.as_ref(),
            &|item, provider| attempts.recent(item, provider, started, retry),
            MAXIMUM_PAID_ACTIONS,
        );
        for action in actions {
            match action {
                Action::Expire { item, provider } => {
                    tracing::debug!(
                        "A replica reached the end of its retention and is no longer counted"
                    );
                    let _ = self.owners.replicas.drop_replica(item, provider);
                    attempts.clear(item, provider);
                }
                Action::Renew {
                    item,
                    provider,
                    lease,
                } => self.renew_replica(item, provider, lease, attempts),
                Action::Check {
                    item,
                    provider,
                    lease,
                    sealed_size,
                } => {
                    self.check_replica(item, provider, lease, sealed_size, attempts);
                }
            }
        }
    }

    // Extend a replica's retention by the wanted retention and record the new expiry.
    fn renew_replica(
        &self,
        item: [u8; 16],
        provider: crate::modules::identity::api::IdentityId,
        lease: [u8; 32],
        attempts: &mut Attempts,
    ) {
        tracing::trace!("Renewing a replica");
        let Some(work) = self.owners.wanted.as_ref() else {
            return;
        };
        let request = request(
            "renew",
            [
                ("lease", json!(hex(&lease))),
                ("seconds", json!(work.terms().retention_seconds)),
                ("maximum", json!(self.owners.policy.redundancy.max_spend())),
            ],
        );
        let expires = self
            .peer_operation(&request)
            .ok()
            .and_then(|v| v["expiresAt"].as_u64());
        match expires {
            Some(expires_at) => {
                let _ = self
                    .owners
                    .replicas
                    .update_replica(item, provider, |r| r.expires_at = expires_at);
                attempts.clear(item, provider);
            }
            None => attempts.note(item, provider, Instant::now()),
        }
    }

    // Check one sampled byte of a replica against the sealed bytes this node keeps.
    fn check_replica(
        &self,
        item: [u8; 16],
        provider: crate::modules::identity::api::IdentityId,
        lease: [u8; 32],
        sealed_size: u64,
        attempts: &mut Attempts,
    ) {
        tracing::trace!("Checking a replica");
        let sample = self.owners.replicas.sealed(item).ok().and_then(|sealed| {
            let offset = sampling::offset(&mut SystemEntropy, sealed_size, 64).ok()?;
            let byte = *sealed.get(usize::try_from(offset).ok()?)?;
            Some((offset, byte))
        });
        let Some((offset, byte)) = sample else {
            attempts.note(item, provider, Instant::now());
            return;
        };
        let request = request(
            "check",
            [
                ("lease", json!(hex(&lease))),
                ("offset", json!(offset)),
                ("expectedByte", json!(hex(&[byte]))),
                ("maximum", json!(self.owners.policy.redundancy.max_spend())),
            ],
        );
        let outcome = self
            .peer_operation(&request)
            .ok()
            .and_then(|v| v["outcome"].as_str().map(str::to_string));
        match outcome.as_deref() {
            Some("pass") => {
                let checked = now();
                let _ = self
                    .owners
                    .replicas
                    .update_replica(item, provider, |r| r.last_checked = Some(checked));
                attempts.clear(item, provider);
            }
            Some("fail") => {
                tracing::warn!("A replica failed a check and is no longer counted");
                let _ = self.owners.replicas.drop_replica(item, provider);
                attempts.clear(item, provider);
            }
            // Unreachable, unpaid or inconclusive: proof of neither presence nor absence.
            _ => attempts.note(item, provider, Instant::now()),
        }
    }
}

// A control request with a fresh identifier and the given parameters.
fn request<const N: usize>(op: &str, params: [(&str, serde_json::Value); N]) -> Request {
    let mut id = [0u8; 8];
    let _ = getrandom::fill(&mut id);
    let mut map = Map::new();
    for (name, value) in params {
        map.insert(name.to_string(), value);
    }
    Request {
        id: u64::from_le_bytes(id),
        op: op.to_string(),
        params: map,
    }
}
