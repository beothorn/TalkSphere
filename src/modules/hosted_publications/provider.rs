//! The hosted publication provider engine: validation, pricing and the durable commit of each operation.
//!
//! Callers: the participant and Services adapter (plan step 12), and tests. Every operation is split in two so the Services coordinator
//! can quote before work and apply once afterwards: an `assess_*` step checks every rule and computes the exact price without changing
//! anything, and a `commit_*` step applies the effect durably at a decision time. The one-shot methods call both. A failure before the
//! commit changes nothing and is uncharged. The authenticated requester is always supplied by the caller, never read from a request.
use super::{
    envelope,
    error::Error,
    ids::PlacementId,
    metadata::Publication,
    operation::{StoreOutput, StoreRequest},
    placement::{Placement, Record, State},
    store::Store,
    terms::Terms,
    verify::{Bounds, verify},
};
use crate::modules::{
    identity::api::IdentityId,
    messaging::ports::{Clock, Entropy, PendingLimits},
};
use std::sync::Arc;

/// A store that passed every check and is priced, ready to be committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePlan {
    /// The authenticated publisher.
    pub publisher: IdentityId,
    /// The verified publication.
    pub publication: Publication,
    /// The agreed initial retention in seconds.
    pub retention_seconds: u64,
    /// The exact store price.
    pub price: u64,
}

/// The provider engine over one placement store.
pub struct Provider {
    terms: Terms,
    store: Arc<Store>,
    clock: Arc<dyn Clock>,
    entropy: Arc<dyn Entropy>,
    limits: Arc<dyn PendingLimits>,
}

impl Provider {
    /// Build an engine from validated terms and its dependencies.
    /// `limits.per_sender` bounds the active placements of one publisher.
    pub fn new(
        terms: Terms,
        store: Arc<Store>,
        clock: Arc<dyn Clock>,
        entropy: Arc<dyn Entropy>,
        limits: Arc<dyn PendingLimits>,
    ) -> Self {
        tracing::trace!("Creating the hosted publication provider");
        Self {
            terms,
            store,
            clock,
            entropy,
            limits,
        }
    }

    /// The validated terms this provider enforces.
    pub fn terms(&self) -> &Terms {
        &self.terms
    }

    /// The placement store, for composition and tests.
    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    /// Check a store request from the authenticated `publisher` and price it, changing nothing.
    ///
    /// The checks are the specification's: the signature rules of [`verify`] against the offering limits, the envelope structure of
    /// restricted content, retention within the offering, room in the finite tables and the publisher's active-placement quota. The
    /// duplicate and same-sequence rules are enforced atomically at commit.
    /// # Errors
    /// `Invalid` for a malformed or unverifiable publication or retention outside the offering; `Limit` when a table or quota is full or
    /// the price overflows; `Unavailable` without a trustworthy clock.
    pub fn assess_store(
        &self,
        publisher: IdentityId,
        request: &StoreRequest,
    ) -> Result<StorePlan, Error> {
        tracing::trace!("Assessing a hosted store request");
        let now = self.clock.now()?;
        let publication = &request.publication;
        let terms = &self.terms;
        let bounds = Bounds {
            maximum_audience: usize::from(terms.maximum_audience),
            maximum_content_bytes: terms.maximum_content_bytes,
        };
        verify(publication, publisher, bounds)?;
        if publication.metadata.visibility == super::metadata::Visibility::Restricted {
            envelope::check_structure(&publication.content, publication.metadata.audience.len())?;
        }
        let retention = request.retention_seconds;
        if retention < terms.minimum_retention_seconds
            || retention > terms.maximum_retention_seconds
            || retention > terms.maximum_total_retention_seconds
            || now.checked_add(retention).is_none()
        {
            return Err(Error::Invalid);
        }
        let size = publication.metadata.content_size;
        if !self.store.has_room(size)
            || self.store.active_count(publisher, now) >= self.limits.per_sender(publisher)
        {
            return Err(Error::Limit);
        }
        Ok(StorePlan {
            publisher,
            publication: publication.clone(),
            retention_seconds: retention,
            price: terms.store_price(size, retention)?,
        })
    }

    /// Commit an assessed store durably under `placement_id` at the decision time `at`, then report it. Repeating the exact commit (same
    /// placement ID and publication) returns the recorded output, so a crash between applying and reporting is repaired by committing
    /// again with the original time.
    /// # Errors
    /// `Conflict` for a duplicate publication or a sequence already used by another publication of this publisher and kind; `Limit` for a
    /// full table; `Storage` when the write fails (nothing is recorded).
    pub fn commit_store(
        &self,
        plan: &StorePlan,
        placement_id: PlacementId,
        at: u64,
    ) -> Result<StoreOutput, Error> {
        tracing::trace!("Committing a hosted store");
        let metadata = &plan.publication.metadata;
        if let Some(existing) = self.store.record(placement_id) {
            let same = existing.metadata == *metadata
                && existing.publisher_signature == plan.publication.publisher_signature;
            return if same {
                Ok(output_of(&existing))
            } else {
                Err(Error::Conflict)
            };
        }
        let expires_at = at
            .checked_add(plan.retention_seconds)
            .ok_or(Error::Invalid)?;
        let record = Record {
            id: placement_id,
            metadata: metadata.clone(),
            publisher_public_key: plan.publication.publisher_public_key,
            publisher_signature: plan.publication.publisher_signature,
            accepted_at: at,
            expires_at,
            state: State::Pending,
            payments: plan.price,
            removal: None,
        };
        self.store.insert(
            &Placement {
                record: record.clone(),
                content: plan.publication.content.clone(),
            },
            at,
        )?;
        Ok(output_of(&record))
    }

    /// Check, price and commit a store in one step with a fresh placement ID at the current time; returns the output and the price.
    /// # Errors
    /// As [`Provider::assess_store`] and [`Provider::commit_store`], plus `Unavailable` without entropy.
    pub fn store_publication(
        &self,
        publisher: IdentityId,
        request: &StoreRequest,
    ) -> Result<(StoreOutput, u64), Error> {
        tracing::trace!("Storing a publication");
        let plan = self.assess_store(publisher, request)?;
        let at = self.clock.now()?;
        let output = self.commit_store(&plan, self.new_placement_id()?, at)?;
        Ok((output, plan.price))
    }

    /// A fresh unpredictable placement identifier that no stored placement uses.
    /// # Errors
    /// `Unavailable` without entropy; never derived from a fallback.
    pub fn new_placement_id(&self) -> Result<PlacementId, Error> {
        tracing::trace!("Choosing a hosted placement identifier");
        loop {
            let mut bytes = [0u8; 32];
            self.entropy.fill(&mut bytes)?;
            let id = PlacementId(bytes);
            if self.store.record(id).is_none() {
                return Ok(id);
            }
        }
    }
}

// The store output of a recorded placement.
fn output_of(record: &Record) -> StoreOutput {
    StoreOutput {
        placement_id: record.id,
        publication_id: record.metadata.publication_id,
        accepted_at: record.accepted_at,
        expires_at: record.expires_at,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::modules::{
        hosted_publications::{
            ids::PublicationId,
            metadata::{Kind, Visibility},
            signing::{Draft, sign_publication, tests::VectorPublisher},
            store::Limits,
            terms::tests::terms,
        },
        messaging::{
            ports::{FixedLimits, Signer},
            test_support::{FixedClock, SequenceEntropy},
        },
    };

    /// A provider over a volatile store, its clock and the vector publisher, shared by the operation tests.
    pub(crate) struct Harness {
        pub(crate) provider: Provider,
        pub(crate) clock: Arc<FixedClock>,
        pub(crate) publisher: VectorPublisher,
    }

    /// The acceptance harness: provider terms of the HP cases, time 1000 and the vector publisher.
    pub(crate) fn harness() -> Harness {
        let clock = Arc::new(FixedClock::at(1000));
        let store = Arc::new(Store::volatile(Limits {
            placements: 16,
            content_bytes: 1000,
        }));
        let provider = Provider::new(
            terms(),
            store,
            clock.clone(),
            Arc::new(SequenceEntropy::new(100)),
            Arc::new(FixedLimits::default()),
        );
        Harness {
            provider,
            clock,
            publisher: VectorPublisher::new(),
        }
    }

    /// A signed public publication by `signer` with the given kind, sequence and content.
    pub(crate) fn public(
        signer: &dyn Signer,
        id: u8,
        kind: Kind,
        sequence: u64,
        content: &[u8],
    ) -> Publication {
        let draft = Draft {
            publication_id: PublicationId([id; 32]),
            kind,
            sequence,
            published_at: 900,
            visibility: Visibility::Public,
            audience: vec![],
        };
        sign_publication(signer, draft, content.to_vec()).unwrap()
    }

    /// A store request with retention `seconds`.
    pub(crate) fn request(publication: Publication, seconds: u64) -> StoreRequest {
        StoreRequest {
            publication,
            retention_seconds: seconds,
        }
    }

    // HP-02: storing `abc` for ten seconds at time 1000 creates a durable placement with a 32-byte ID, acceptance 1000, expiry 1010
    // and the price of 17 from the formula, and the placement holds exactly the signed bytes.
    #[test]
    fn a_store_creates_the_priced_durable_placement() {
        let h = harness();
        let publication = public(&h.publisher, 1, Kind::Resource, 0, b"abc");
        let (output, price) = h
            .provider
            .store_publication(h.publisher.identity(), &request(publication.clone(), 10))
            .unwrap();
        assert_eq!(
            (output.accepted_at, output.expires_at, price),
            (1000, 1010, 17)
        );
        assert_eq!(output.publication_id, PublicationId([1; 32]));
        let stored = h
            .provider
            .store()
            .get(output.placement_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.publication(), publication);
        assert_eq!(stored.record.payments, 17);
        // A second store of the same bytes is a duplicate publication, never silently a new placement.
        assert_eq!(
            h.provider
                .store_publication(h.publisher.identity(), &request(publication, 10))
                .err(),
            Some(Error::Conflict)
        );
    }

    // Repeating the exact commit with the original decision time returns the recorded output without a second placement (crash repair),
    // while the same placement ID for a different publication is a conflict.
    #[test]
    fn committing_again_is_idempotent() {
        let h = harness();
        let me = h.publisher.identity();
        let plan = h
            .provider
            .assess_store(
                me,
                &request(public(&h.publisher, 1, Kind::Page, 1, b"abc"), 10),
            )
            .unwrap();
        let id = h.provider.new_placement_id().unwrap();
        let first = h.provider.commit_store(&plan, id, 1000).unwrap();
        h.clock.advance(50);
        assert_eq!(h.provider.commit_store(&plan, id, 1000), Ok(first));
        assert_eq!(h.provider.store().records().len(), 1);
        let other = h
            .provider
            .assess_store(
                me,
                &request(public(&h.publisher, 2, Kind::Page, 2, b"abc"), 10),
            )
            .unwrap();
        assert_eq!(
            h.provider.commit_store(&other, id, 1050),
            Err(Error::Conflict)
        );
    }

    // HP-03: every rejection leaves no placement; retention bounds, price overflow limits and the unavailable clock are distinct.
    #[test]
    fn rejections_change_nothing() {
        let h = harness();
        let me = h.publisher.identity();
        let good = public(&h.publisher, 1, Kind::Page, 1, b"abc");
        let mut bad_sig = good.clone();
        bad_sig.publisher_signature.0[0] ^= 1;
        let mut bad_content = good.clone();
        bad_content.content[0] ^= 1;
        let mut wrong_size = good.clone();
        wrong_size.metadata.content_size = 4;
        let mut resource_sequence = public(&h.publisher, 2, Kind::Resource, 0, b"abc");
        resource_sequence.metadata.sequence = 1;
        let mut public_audience = good.clone();
        public_audience.metadata.audience = vec![IdentityId([9; 32])];
        let too_big = public(&h.publisher, 3, Kind::Resource, 0, &[1u8; 101]);
        let cases = [
            (bad_sig, 10),
            (bad_content, 10),
            (wrong_size, 10),
            (resource_sequence, 10),
            (public_audience, 10),
            (too_big, 10),
            (good.clone(), 0),
            (good.clone(), 101),
            (good.clone(), u64::MAX),
        ];
        for (publication, seconds) in cases {
            assert_eq!(
                h.provider
                    .store_publication(me, &request(publication, seconds))
                    .err(),
                Some(Error::Invalid)
            );
        }
        // Another authenticated caller cannot store the publisher's publication.
        assert_eq!(
            h.provider
                .store_publication(IdentityId([1; 32]), &request(good.clone(), 10))
                .err(),
            Some(Error::Invalid)
        );
        assert!(h.provider.store().records().is_empty());
        h.clock.fail();
        assert_eq!(
            h.provider.store_publication(me, &request(good, 10)).err(),
            Some(Error::Unavailable)
        );
    }

    // A restricted publication must carry an envelope whose entry count equals the audience; a plain value is refused.
    #[test]
    fn restricted_content_needs_a_matching_envelope() {
        let h = harness();
        let me = h.publisher.identity();
        let draft = |content: &[u8]| {
            let d = Draft {
                publication_id: PublicationId([4; 32]),
                kind: Kind::FeedPost,
                sequence: 1,
                published_at: 900,
                visibility: Visibility::Restricted,
                audience: vec![IdentityId([9; 32])],
            };
            sign_publication(&h.publisher, d, content.to_vec()).unwrap()
        };
        // The independent V3 envelope is 295 bytes, above this offering's 100-byte maximum, so use a bounded one.
        let key =
            x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from([4; 32])).to_bytes();
        let mut metadata = draft(b"x").metadata;
        metadata.content_size = 0;
        let sealed = envelope::seal(b"hi", &metadata, &[key], &SequenceEntropy::new(1)).unwrap();
        assert_eq!(
            sealed.len(),
            envelope::HEADER_BYTES + envelope::ENTRY_BYTES + 2 + 16
        );
        assert!(sealed.len() > 100);
        let wide = Terms {
            maximum_content_bytes: 200,
            ..terms()
        };
        let provider = Provider::new(
            wide,
            Arc::new(Store::volatile(Limits {
                placements: 4,
                content_bytes: 1000,
            })),
            h.clock.clone(),
            Arc::new(SequenceEntropy::new(7)),
            Arc::new(FixedLimits::default()),
        );
        assert_eq!(
            provider
                .store_publication(me, &request(draft(b"plain value"), 10))
                .err(),
            Some(Error::Invalid)
        );
        assert!(
            provider
                .store_publication(me, &request(draft(&sealed), 10))
                .is_ok()
        );
    }

    // Quotas and tables are finite: a publisher's active placements, the placement count and the content bytes each give `Limit`, and a
    // lapsed placement stops counting against the quota.
    #[test]
    fn quotas_and_tables_are_finite() {
        let h = harness();
        let me = h.publisher.identity();
        let quota = Provider::new(
            terms(),
            Arc::new(Store::volatile(Limits {
                placements: 4,
                content_bytes: 10,
            })),
            h.clock.clone(),
            Arc::new(SequenceEntropy::new(9)),
            Arc::new(FixedLimits {
                per_sender: 2,
                per_recipient: 2,
            }),
        );
        for n in 1..=2u8 {
            quota
                .store_publication(
                    me,
                    &request(public(&h.publisher, n, Kind::Resource, 0, b"abc"), 10),
                )
                .unwrap();
        }
        let third = request(public(&h.publisher, 3, Kind::Resource, 0, b"abc"), 10);
        assert_eq!(
            quota.store_publication(me, &third).err(),
            Some(Error::Limit)
        );
        h.clock.advance(10);
        assert!(quota.store_publication(me, &third).is_ok());
        let bytes = request(public(&h.publisher, 4, Kind::Resource, 0, &[2u8; 9]), 10);
        assert_eq!(
            quota.store_publication(me, &bytes).err(),
            Some(Error::Limit)
        );
    }

    // A failed durable write records nothing and reports `Storage`; an unavailable entropy source reports `Unavailable` and no ID is invented.
    #[test]
    fn dependency_failures_are_reported_and_record_nothing() {
        let h = harness();
        let me = h.publisher.identity();
        let publication = public(&h.publisher, 1, Kind::Page, 1, b"abc");
        h.provider.store().fail_writes(true);
        assert_eq!(
            h.provider
                .store_publication(me, &request(publication.clone(), 10))
                .err(),
            Some(Error::Storage)
        );
        h.provider.store().fail_writes(false);
        assert!(h.provider.store().records().is_empty());
        let silent = SequenceEntropy::new(1);
        silent.fail();
        let broken = Provider::new(
            terms(),
            h.provider.store().clone(),
            h.clock.clone(),
            Arc::new(silent),
            Arc::new(FixedLimits::default()),
        );
        assert_eq!(
            broken
                .store_publication(me, &request(publication, 10))
                .err(),
            Some(Error::Unavailable)
        );
        assert!(h.provider.store().records().is_empty());
    }
}
