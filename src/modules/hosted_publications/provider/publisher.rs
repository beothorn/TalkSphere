//! Publisher-only operations of the provider: renew, remove with early-release credits, and the byte check.
//!
//! Callers: the participant and Services adapter (plan step 12) and tests. Only the publisher of a placement may use these. A placement
//! that is unknown, belongs to another publisher, is not pending or has expired is reported as one concealed `NotAvailable`, so a
//! caller learns nothing about other publishers' placements. Renewal and removal are split into an assessment that changes nothing and
//! a commit that applies the effect idempotently at the coordinator's decision time.
use super::Provider;
use crate::modules::{
    hosted_publications::{
        error::Error,
        ids::{PlacementId, RemovalId},
        lifecycle,
        operation::{
            CheckOutput, CheckRequest, RemoveOutput, RemoveRequest, RenewOutput, RenewRequest,
        },
        placement::{Record, Removal, State},
    },
    identity::api::IdentityId,
    services::contract::OperationKey,
};

/// A renewal that passed every check and is priced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenewPlan {
    /// The placement to extend.
    pub placement_id: PlacementId,
    /// The expiry the renewal requires.
    pub expected_expires_at: u64,
    /// Seconds to add.
    pub additional_seconds: u64,
    /// The latest allowed expiry: acceptance plus the offering's total retention.
    pub ceiling_expires_at: u64,
    /// The exact renewal price.
    pub price: u64,
}

/// The terms of one removal, fixed before anything changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemovalQuote {
    /// The exact early-release credits that will be issued to the publisher.
    pub release_credits: u64,
    /// The removal charge.
    pub price: u64,
}

/// What a removal request needs: an answer from the record for an exact retry, or fresh terms to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovePlan {
    /// This exact removal already completed; repeat its recorded output with nothing applied again.
    Replay(RemoveOutput),
    /// Apply the removal under these terms.
    Fresh {
        /// The request being applied.
        request: RemoveRequest,
        /// The agreed terms.
        quote: RemovalQuote,
    },
}

impl Provider {
    /// Check a renewal from the authenticated `publisher` and price it, changing nothing.
    ///
    /// The placement must be pending, unexpired and the publisher's own; the added time must be within the offering's retention limits;
    /// the new expiry `expected + additional` must not pass the acceptance time plus the total retention. The expectation itself is
    /// enforced atomically at commit.
    /// # Errors
    /// `Invalid` for added time outside the offering; `NotAvailable` as above; `Limit` for a passed ceiling or overflow.
    pub fn assess_renew(
        &self,
        publisher: IdentityId,
        request: &RenewRequest,
    ) -> Result<RenewPlan, Error> {
        tracing::trace!("Assessing a hosted renewal");
        let terms = &self.terms;
        let seconds = request.additional_seconds;
        if seconds < terms.minimum_retention_seconds || seconds > terms.maximum_retention_seconds {
            return Err(Error::Invalid);
        }
        let now = self.clock.now()?;
        let record = self.own_active(publisher, request.placement_id, now)?;
        // Checked here, not only at commit, so a second renewal from an expectation that is already used is a conflict and never mistaken
        // for the repetition of an applied one.
        if record.expires_at != request.expected_expires_at {
            return Err(Error::Conflict);
        }
        let ceiling_expires_at = record
            .accepted_at
            .checked_add(terms.maximum_total_retention_seconds)
            .ok_or(Error::Limit)?;
        let target = request
            .expected_expires_at
            .checked_add(seconds)
            .ok_or(Error::Limit)?;
        if target > ceiling_expires_at {
            return Err(Error::Limit);
        }
        Ok(RenewPlan {
            placement_id: request.placement_id,
            expected_expires_at: request.expected_expires_at,
            additional_seconds: seconds,
            ceiling_expires_at,
            price: terms.renew_price(record.metadata.content_size, seconds)?,
        })
    }

    /// Apply an assessed renewal at the decision time `at`. The expiry only ever grows, so a later expiry means this renewal (or a later
    /// one) was already applied and its output is rebuilt: repeating the commit is harmless.
    /// # Errors
    /// `Conflict` for a stale expected expiry, `NotAvailable` when no longer active, `Limit` for overflow or a passed ceiling.
    pub fn commit_renew(&self, plan: &RenewPlan, at: u64) -> Result<(RenewOutput, u64), Error> {
        tracing::trace!("Committing a hosted renewal");
        let target = plan
            .expected_expires_at
            .checked_add(plan.additional_seconds)
            .ok_or(Error::Limit)?;
        let record = self
            .store
            .record(plan.placement_id)
            .ok_or(Error::NotAvailable)?;
        if record.expires_at < target {
            lifecycle::extend(
                &self.store,
                plan.placement_id,
                at,
                plan.expected_expires_at,
                plan.additional_seconds,
                plan.ceiling_expires_at,
                plan.price,
            )?;
        }
        Ok((
            RenewOutput {
                placement_id: plan.placement_id,
                previous_expires_at: plan.expected_expires_at,
                expires_at: target,
            },
            plan.price,
        ))
    }

    /// Check, price and apply a renewal in one step at the current time.
    /// # Errors
    /// As [`Provider::assess_renew`] and [`Provider::commit_renew`].
    pub fn renew(
        &self,
        publisher: IdentityId,
        request: &RenewRequest,
    ) -> Result<(RenewOutput, u64), Error> {
        tracing::trace!("Renewing a placement for its publisher");
        let _serial = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        let plan = self.assess_renew(publisher, request)?;
        let at = self.clock.now()?;
        self.commit_renew(&plan, at)
    }

    /// Quote a removal: one exact release amount from the policy, bounded below by the publisher's minimum and above by the placement's
    /// cumulative store and renewal payments. List, get, remove and check prices are never added to those payments, so they cannot
    /// enlarge the ceiling. Nothing changes.
    /// # Errors
    /// `NotAvailable` for an unknown, foreign, inactive or expired placement; `Conflict` for a stale expiry; `Invalid` when the minimum
    /// exceeds the payments; `Limit` when the policy offers less than the minimum.
    pub fn quote_removal(
        &self,
        publisher: IdentityId,
        request: &RemoveRequest,
    ) -> Result<RemovalQuote, Error> {
        tracing::trace!("Quoting early-release credits for a removal");
        let now = self.clock.now()?;
        let record = self.own_active(publisher, request.placement_id, now)?;
        if record.expires_at != request.expected_expires_at {
            return Err(Error::Conflict);
        }
        let payments = record.payments;
        if request.minimum_release_credits > payments {
            return Err(Error::Invalid);
        }
        let release_credits = self
            .release
            .release(payments, request.minimum_release_credits)
            .min(payments);
        if release_credits < request.minimum_release_credits {
            return Err(Error::Limit);
        }
        Ok(RemovalQuote {
            release_credits,
            price: self.terms.remove_price,
        })
    }

    /// Validate a removal without changing anything: an exact retry (the placement removed with the same removal ID) is answered from the
    /// durable record, with no expiry check, so a lost reply can be recovered for as long as the record is kept; otherwise the quote fixes
    /// the credits. A caller that separates assessment from commit must keep the placement from changing in between (the Services
    /// participant locks the placement); [`Provider::remove`] does it with an internal lock.
    /// # Errors
    /// Those of [`Provider::quote_removal`].
    pub fn assess_remove(
        &self,
        publisher: IdentityId,
        request: &RemoveRequest,
    ) -> Result<RemovePlan, Error> {
        tracing::trace!("Assessing a hosted removal");
        if let Some(done) = self.recorded_removal(publisher, request) {
            return Ok(RemovePlan::Replay(done));
        }
        Ok(RemovePlan::Fresh {
            request: *request,
            quote: self.quote_removal(publisher, request)?,
        })
    }

    /// Apply a removal under `key` at the decision time `at`: with the placement's state and the fixed time checked first, issue the
    /// early-release credits once under the key, then record the removal and release the content. Repeating the commit after the removal
    /// was recorded issues again idempotently, so a settlement interrupted between its records is completed exactly once.
    /// # Errors
    /// `NotAvailable` when no longer pending and unexpired, `Conflict` for a stale expiry, `Unsupported` when credits are released but no
    /// issuer is installed, or the issuer's refusal; a refused issuance leaves the placement pending.
    pub fn commit_remove(
        &self,
        key: OperationKey,
        request: &RemoveRequest,
        quote: RemovalQuote,
        at: u64,
    ) -> Result<RemoveOutput, Error> {
        tracing::trace!("Committing a hosted removal");
        let record = self
            .store
            .record(request.placement_id)
            .ok_or(Error::NotAvailable)?;
        let publisher = record.metadata.publisher_id;
        if let (State::Removed, Some(done)) = (record.state, record.removal)
            && done.removal_id == request.removal_id
        {
            self.issue_release(key, publisher, done.release_credits)?;
            return Ok(removed(request.placement_id, done));
        }
        if !record.active_at(at) {
            return Err(Error::NotAvailable);
        }
        if record.expires_at != request.expected_expires_at {
            return Err(Error::Conflict);
        }
        let removal = Removal {
            removal_id: request.removal_id,
            release_credits: quote.release_credits,
            price: quote.price,
        };
        self.issue_release(key, publisher, quote.release_credits)?;
        lifecycle::mark_removed(
            &self.store,
            request.placement_id,
            at,
            request.expected_expires_at,
            removal,
        )?;
        self.store.release_content(request.placement_id)?;
        Ok(removed(request.placement_id, removal))
    }

    /// Assess and apply a removal in one step at the current time.
    /// # Errors
    /// As [`Provider::assess_remove`] and [`Provider::commit_remove`].
    pub fn remove(
        &self,
        key: OperationKey,
        publisher: IdentityId,
        request: &RemoveRequest,
    ) -> Result<RemoveOutput, Error> {
        tracing::trace!("Removing a pending placement for its publisher");
        let _serial = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        match self.assess_remove(publisher, request)? {
            RemovePlan::Replay(done) => Ok(done),
            RemovePlan::Fresh { request, quote } => {
                let at = self.clock.now()?;
                self.commit_remove(key, &request, quote, at)
            }
        }
    }

    /// Serve one content byte of a pending placement to its publisher as paid evidence that the host still holds the publication.
    /// Read-only; the check price is charged by the Services layer only for a complete valid response.
    /// # Errors
    /// `NotAvailable` for an unknown, foreign, inactive or expired placement, concealed alike; `Invalid` for an offset at or beyond the
    /// content size; `Storage` when the content cannot be read.
    pub fn check(
        &self,
        publisher: IdentityId,
        request: &CheckRequest,
    ) -> Result<CheckOutput, Error> {
        tracing::trace!("Serving one content byte of evidence to the publisher");
        let now = self.clock.now()?;
        let record = self.own_active(publisher, request.placement_id, now)?;
        if request.offset >= record.metadata.content_size {
            return Err(Error::Invalid);
        }
        let placement = self
            .store
            .get(request.placement_id)?
            .ok_or(Error::NotAvailable)?;
        let index = usize::try_from(request.offset).map_err(|_| Error::Invalid)?;
        let value = *placement.content.get(index).ok_or(Error::Invalid)?;
        Ok(CheckOutput {
            placement_id: request.placement_id,
            offset: request.offset,
            value,
        })
    }

    // The record of a pending, unexpired placement owned by `publisher`; everything else is one concealed failure.
    fn own_active(
        &self,
        publisher: IdentityId,
        id: PlacementId,
        now: u64,
    ) -> Result<Record, Error> {
        self.store
            .record(id)
            .filter(|r| r.active_at(now) && r.metadata.publisher_id == publisher)
            .ok_or(Error::NotAvailable)
    }

    // The recorded result of this exact removal, if the publisher repeats it: the placement is removed with the same removal ID.
    fn recorded_removal(
        &self,
        publisher: IdentityId,
        request: &RemoveRequest,
    ) -> Option<RemoveOutput> {
        let record = self.store.record(request.placement_id)?;
        let removal = record.removal?;
        if record.state != State::Removed
            || record.metadata.publisher_id != publisher
            || removal.removal_id != request.removal_id
        {
            return None;
        }
        Some(removed(record.id, removal))
    }

    // Issue the early-release credits once; none are issued for zero credits, and a positive amount without an issuer is refused because
    // the capability is not installed.
    fn issue_release(
        &self,
        key: OperationKey,
        publisher: IdentityId,
        credits: u64,
    ) -> Result<(), Error> {
        if credits == 0 {
            return Ok(());
        }
        let issuer = self.issuer.as_ref().ok_or(Error::Unsupported)?;
        Ok(issuer.issue(key, publisher, credits)?)
    }
}

// The output of a recorded removal.
fn removed(placement_id: PlacementId, removal: Removal) -> RemoveOutput {
    RemoveOutput {
        placement_id,
        removal_id: RemovalId(removal.removal_id.0),
        removed: true,
        release_credits: removal.release_credits,
        price: removal.price,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{
        hosted_publications::{
            metadata::Kind,
            provider::tests::{Harness, harness, public, request},
            store::Limits,
        },
        messaging::{
            ports::FixedLimits,
            ports::Signer,
            test_support::{RecordingIssuer, SequenceEntropy},
        },
    };
    use std::sync::Arc;

    fn key(n: u8) -> OperationKey {
        OperationKey {
            requester: IdentityId([n; 32]),
            connection: [n; 32],
            stream: u32::from(n),
        }
    }

    // A provider that offers `percent` percent of the payments as release credits and records what it issues.
    fn removing(h: &Harness, percent: u64) -> (Provider, Arc<RecordingIssuer>) {
        let issuer = Arc::new(RecordingIssuer::default());
        let provider = Provider::new(
            h.provider.terms,
            Arc::new(crate::modules::hosted_publications::store::Store::volatile(
                Limits {
                    placements: 16,
                    content_bytes: 1000,
                },
            )),
            h.clock.clone(),
            Arc::new(SequenceEntropy::new(200)),
            Arc::new(FixedLimits::default()),
        )
        .with_release_policy(Arc::new(move |payments: u64, _min: u64| {
            payments * percent / 100
        }))
        .with_release_issuer(issuer.clone());
        (provider, issuer)
    }

    // Store `abc` for ten seconds (price 17, expiring at 1010) and return its placement.
    fn stored(h: &Harness, provider: &Provider) -> PlacementId {
        let publication = public(&h.publisher, 1, Kind::Page, 1, b"abc");
        provider
            .store_publication(h.publisher.identity(), &request(publication, 10))
            .unwrap()
            .0
            .placement_id
    }

    // HP-11: renewing 3 bytes by 10 s from the exact expiry extends it, charges 15 and adds it to the payments; every refusal changes nothing.
    #[test]
    fn renewal_extends_from_the_exact_expectation() {
        let h = harness();
        let me = h.publisher.identity();
        let id = stored(&h, &h.provider);
        let ask = |expected, seconds| RenewRequest {
            placement_id: id,
            expected_expires_at: expected,
            additional_seconds: seconds,
        };
        let (output, price) = h.provider.renew(me, &ask(1010, 10)).unwrap();
        assert_eq!(
            (output.previous_expires_at, output.expires_at, price),
            (1010, 1020, 15)
        );
        assert_eq!(h.provider.store().record(id).unwrap().payments, 32);
        // Stale expectation, duration outside the offering, passed ceiling (acceptance 1000 + 200), non-publisher: all unchanged.
        assert_eq!(
            h.provider.renew(me, &ask(1010, 10)).err(),
            Some(Error::Conflict)
        );
        assert_eq!(
            h.provider.renew(me, &ask(1020, 0)).err(),
            Some(Error::Invalid)
        );
        assert_eq!(
            h.provider.renew(me, &ask(1020, 101)).err(),
            Some(Error::Invalid)
        );
        assert_eq!(
            h.provider.renew(me, &ask(1020, 100)).unwrap().0.expires_at,
            1120
        );
        assert_eq!(
            h.provider.renew(me, &ask(1120, 81)).err(),
            Some(Error::Limit)
        );
        assert_eq!(
            h.provider.renew(me, &ask(1120, 80)).unwrap().0.expires_at,
            1200
        );
        assert_eq!(
            h.provider.renew(me, &ask(1200, 1)).err(),
            Some(Error::Limit)
        );
        assert_eq!(
            h.provider.renew(IdentityId([5; 32]), &ask(1200, 1)).err(),
            Some(Error::NotAvailable)
        );
        assert_eq!(h.provider.store().record(id).unwrap().expires_at, 1200);
        h.clock.set(1200);
        assert_eq!(
            h.provider.renew(me, &ask(1200, 1)).err(),
            Some(Error::NotAvailable)
        );
    }

    // Repeating a renewal commit after it was applied (crash repair) rebuilds the output without charging or extending twice.
    #[test]
    fn renewal_commits_are_idempotent() {
        let h = harness();
        let me = h.publisher.identity();
        let id = stored(&h, &h.provider);
        let plan = h
            .provider
            .assess_renew(
                me,
                &RenewRequest {
                    placement_id: id,
                    expected_expires_at: 1010,
                    additional_seconds: 10,
                },
            )
            .unwrap();
        let first = h.provider.commit_renew(&plan, 1000).unwrap();
        assert_eq!(h.provider.commit_renew(&plan, 1001).unwrap(), first);
        assert_eq!(h.provider.store().record(id).unwrap().expires_at, 1020);
        assert_eq!(h.provider.store().record(id).unwrap().payments, 32);
    }

    // HP-12: removal with the quoted credits ends the placement, charges 2, issues 5 once, keeps the replay record, answers an exact retry
    // from it without a second release, refuses another removal ID and stops reads.
    #[test]
    fn removal_releases_credits_once_and_replays() {
        let h = harness();
        let me = h.publisher.identity();
        let (provider, issuer) = removing(&h, 30);
        let id = stored(&h, &provider);
        let ask = RemoveRequest {
            placement_id: id,
            removal_id: RemovalId([3; 32]),
            expected_expires_at: 1010,
            minimum_release_credits: 5,
        };
        // 30 percent of the 17 credits paid is 5.
        let quote = provider.quote_removal(me, &ask).unwrap();
        assert_eq!(
            quote,
            RemovalQuote {
                release_credits: 5,
                price: 2
            }
        );
        let done = provider.remove(key(1), me, &ask).unwrap();
        assert_eq!(
            (done.removed, done.release_credits, done.price),
            (true, 5, 2)
        );
        assert_eq!(issuer.issued(), vec![(key(1), me, 5)]);
        let record = provider.store().record(id).unwrap();
        assert_eq!(record.state, State::Removed);
        assert_eq!(provider.store().get(id).err(), Some(Error::Storage));
        // The exact retry, even after the original expiry, returns the recorded output and issues nothing more.
        h.clock.advance(500);
        assert_eq!(provider.remove(key(2), me, &ask).unwrap(), done);
        assert_eq!(issuer.issued().len(), 1);
        let other = RemoveRequest {
            removal_id: RemovalId([4; 32]),
            ..ask
        };
        assert_eq!(
            provider.remove(key(3), me, &other).err(),
            Some(Error::NotAvailable)
        );
        assert_eq!(provider.store().active_count(me, 1500), 0);
    }

    // The quote rules: an unreachable minimum, a policy offering less, a stale expiry and strangers are refused; the ceiling is the payments
    // (not later list, get or remove prices); a repeated commit after the removal was recorded re-issues idempotently.
    #[test]
    fn quotes_are_bounded_and_commits_repair_crashes() {
        let h = harness();
        let me = h.publisher.identity();
        let (provider, issuer) = removing(&h, 30);
        let id = stored(&h, &provider);
        let ask = RemoveRequest {
            placement_id: id,
            removal_id: RemovalId([3; 32]),
            expected_expires_at: 1010,
            minimum_release_credits: 0,
        };
        assert_eq!(
            provider
                .quote_removal(
                    me,
                    &RemoveRequest {
                        minimum_release_credits: 18,
                        ..ask
                    }
                )
                .err(),
            Some(Error::Invalid)
        );
        assert_eq!(
            provider
                .quote_removal(
                    me,
                    &RemoveRequest {
                        minimum_release_credits: 6,
                        ..ask
                    }
                )
                .err(),
            Some(Error::Limit)
        );
        assert_eq!(
            provider
                .quote_removal(
                    me,
                    &RemoveRequest {
                        expected_expires_at: 1011,
                        ..ask
                    }
                )
                .err(),
            Some(Error::Conflict)
        );
        assert_eq!(
            provider.quote_removal(IdentityId([5; 32]), &ask).err(),
            Some(Error::NotAvailable)
        );
        assert_eq!(
            provider
                .quote_removal(
                    me,
                    &RemoveRequest {
                        placement_id: PlacementId([9; 32]),
                        ..ask
                    }
                )
                .err(),
            Some(Error::NotAvailable)
        );
        let quote = provider.quote_removal(me, &ask).unwrap();
        provider.commit_remove(key(1), &ask, quote, 1000).unwrap();
        // The coordinator may deliver the same decision again after a crash: still one release.
        provider.commit_remove(key(1), &ask, quote, 1000).unwrap();
        assert_eq!(issuer.issued().len(), 1);
        // A refused issuance leaves the placement pending and the effect retryable.
        let id2 = provider
            .store_publication(
                me,
                &request(public(&h.publisher, 2, Kind::Page, 2, b"abc"), 10),
            )
            .unwrap()
            .0
            .placement_id;
        let ask2 = RemoveRequest {
            placement_id: id2,
            removal_id: RemovalId([6; 32]),
            ..ask
        };
        issuer.fail_with(Some(crate::modules::messaging::Error::Insufficient));
        assert_eq!(
            provider.remove(key(2), me, &ask2).err(),
            Some(Error::Insufficient)
        );
        assert_eq!(provider.store().record(id2).unwrap().state, State::Pending);
        issuer.fail_with(None);
        assert!(provider.remove(key(2), me, &ask2).is_ok());
    }

    // Credits need an installed issuer; without one a releasing removal is unsupported and a zero-credit removal needs none.
    #[test]
    fn releasing_without_an_issuer_is_unsupported() {
        let h = harness();
        let me = h.publisher.identity();
        let id = stored(&h, &h.provider);
        let plain = RemoveRequest {
            placement_id: id,
            removal_id: RemovalId([3; 32]),
            expected_expires_at: 1010,
            minimum_release_credits: 0,
        };
        // The default policy offers nothing, so this removal releases zero credits and succeeds.
        assert_eq!(
            h.provider
                .remove(key(1), me, &plain)
                .unwrap()
                .release_credits,
            0
        );
        let provider = Provider::new(
            h.provider.terms,
            Arc::new(crate::modules::hosted_publications::store::Store::volatile(
                Limits {
                    placements: 4,
                    content_bytes: 100,
                },
            )),
            h.clock.clone(),
            Arc::new(SequenceEntropy::new(300)),
            Arc::new(FixedLimits::default()),
        )
        .with_release_policy(Arc::new(|payments: u64, _: u64| payments));
        let id = stored(&h, &provider);
        let ask = RemoveRequest {
            placement_id: id,
            ..plain
        };
        assert_eq!(
            provider.remove(key(2), me, &ask).err(),
            Some(Error::Unsupported)
        );
    }

    // HP-13: the publisher gets exactly the requested content byte while pending, an offset at the size is invalid, and strangers, other
    // placements, expired and removed placements are one concealed failure; a lost content is a storage failure.
    #[test]
    fn check_serves_one_byte_to_the_publisher_only() {
        let h = harness();
        let me = h.publisher.identity();
        let id = stored(&h, &h.provider);
        let ask = |offset| CheckRequest {
            placement_id: id,
            offset,
        };
        assert_eq!(h.provider.check(me, &ask(1)).unwrap().value, b'b');
        assert_eq!(h.provider.check(me, &ask(3)).err(), Some(Error::Invalid));
        assert_eq!(
            h.provider.check(IdentityId([5; 32]), &ask(1)).err(),
            Some(Error::NotAvailable)
        );
        assert_eq!(
            h.provider
                .check(
                    me,
                    &CheckRequest {
                        placement_id: PlacementId([9; 32]),
                        offset: 0
                    }
                )
                .err(),
            Some(Error::NotAvailable)
        );
        h.provider.store().release_content(id).unwrap();
        assert_eq!(h.provider.check(me, &ask(1)).err(), Some(Error::Storage));
        h.clock.set(1010);
        assert_eq!(
            h.provider.check(me, &ask(1)).err(),
            Some(Error::NotAvailable)
        );
    }

    // HP-14: a reader cannot renew, remove or check a publisher's placement, and the failure is the same as for an unknown placement.
    #[test]
    fn only_the_publisher_may_change_or_check() {
        let h = harness();
        let me = h.publisher.identity();
        let reader = IdentityId([8; 32]);
        let id = stored(&h, &h.provider);
        let unknown = PlacementId([9; 32]);
        let renew = |who, p| {
            h.provider
                .renew(
                    who,
                    &RenewRequest {
                        placement_id: p,
                        expected_expires_at: 1010,
                        additional_seconds: 5,
                    },
                )
                .err()
        };
        let remove = |who, p| {
            h.provider
                .remove(
                    key(1),
                    who,
                    &RemoveRequest {
                        placement_id: p,
                        removal_id: RemovalId([1; 32]),
                        expected_expires_at: 1010,
                        minimum_release_credits: 0,
                    },
                )
                .err()
        };
        let check = |who, p| {
            h.provider
                .check(
                    who,
                    &CheckRequest {
                        placement_id: p,
                        offset: 0,
                    },
                )
                .err()
        };
        assert_eq!(renew(reader, id), renew(reader, unknown));
        assert_eq!(remove(reader, id), remove(reader, unknown));
        assert_eq!(check(reader, id), check(reader, unknown));
        assert_eq!(check(reader, id), Some(Error::NotAvailable));
        assert!(check(me, id).is_none());
    }

    // HP-11/HP-12: eight threads racing a removal and renewals from one expectation produce one winner and a consistent placement.
    #[test]
    fn racing_renewals_and_removal_have_one_winner() {
        for _ in 0..10 {
            let h = harness();
            let me = h.publisher.identity();
            let (provider, _issuer) = removing(&h, 0);
            let provider = Arc::new(provider);
            let id = stored(&h, &provider);
            let mut handles = Vec::new();
            for n in 0..8u8 {
                let provider = Arc::clone(&provider);
                handles.push(std::thread::spawn(move || {
                    if n % 2 == 0 {
                        let ask = RemoveRequest {
                            placement_id: id,
                            removal_id: RemovalId([n + 1; 32]),
                            expected_expires_at: 1010,
                            minimum_release_credits: 0,
                        };
                        provider.remove(key(n), me, &ask).is_ok()
                    } else {
                        provider
                            .renew(
                                me,
                                &RenewRequest {
                                    placement_id: id,
                                    expected_expires_at: 1010,
                                    additional_seconds: 5,
                                },
                            )
                            .is_ok()
                    }
                }));
            }
            let winners = handles
                .into_iter()
                .filter_map(|t| t.join().ok())
                .filter(|w| *w)
                .count();
            assert_eq!(winners, 1);
            let record = provider.store().record(id).unwrap();
            assert!(matches!(
                (record.state, record.expires_at),
                (State::Removed, 1010) | (State::Pending, 1015)
            ));
        }
    }
}
