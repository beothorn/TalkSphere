//! Rebuild finite original-key holds from complete durable Storage history, without assuming rollback or eviction.
use crate::modules::{
    services::{self, Error},
    storage::{
        contract::Limits,
        effect::Effect,
        image::{Image, Stage},
        quota::{Quota, Usage},
    },
};
use std::collections::BTreeSet;

// Count retained output encodings and actual result buffers; original create records retain byte/identity obligations.
pub(super) fn rebuild(
    image: &Image,
    image_limits: super::super::image::Limits,
    limits: Limits,
) -> Result<Quota, Error> {
    tracing::trace!("Reconstructing original Storage resource holds before admission");
    let quota = Quota::new(limits)?;
    let mut committed = BTreeSet::new();
    for record in &image.operations {
        if matches!(record.status, Stage::Staged | Stage::Prepared(_)) {
            return Err(Error::Unknown);
        }
        let usage = record_usage(image, record, image_limits)?;
        if let Effect::Create(original) = Effect::decode(
            record.completion.effect.bytes(),
            image.provider,
            image_limits,
        )? && matches!(record.status, Stage::Committed(_))
        {
            let lease = image
                .leases
                .iter()
                .find(|l| l.id == original.id)
                .ok_or(Error::Invalid)?;
            if !committed.insert(lease.id)
                || lease.provider != original.provider
                || lease.requester != original.requester
                || lease.size_bytes != original.size_bytes
                || lease.content_hash != original.content_hash
                || lease.accepted_at != original.accepted_at
                || lease.accepted_terms != original.accepted_terms
                || lease.store_price != original.store_price
            {
                return Err(Error::Invalid);
            }
        }
        quota.reserve(record.key, usage)?;
    }
    if committed.len() != image.leases.len() {
        return Err(Error::Invalid);
    }
    Ok(quota)
}

// Keep live-phase transitions and startup reconstruction on the same conservative resource accounting rules.
pub(super) fn record_usage(
    image: &Image,
    record: &crate::modules::storage::image::Retained,
    image_limits: super::super::image::Limits,
) -> Result<Usage, Error> {
    tracing::trace!("Calculating retained Storage operation resource usage");
    let encoded = services::codec::encode(
        &record.completion.output,
        image_limits.values,
        image_limits.bytes,
    )?;
    let result_bytes = u64::try_from(
        encoded
            .len()
            .checked_add(record.completion.result.bytes().len())
            .ok_or(Error::Limit)?,
    )
    .map_err(|_| Error::Limit)?;
    let pending = matches!(record.status, Stage::Staged | Stage::Prepared(_));
    let mut usage = Usage {
        result_bytes,
        operations: usize::from(pending),
        ..Usage::default()
    };
    if let Effect::Create(original) = Effect::decode(
        record.completion.effect.bytes(),
        image.provider,
        image_limits,
    )? {
        // Terminal/orphan bytes stay charged until owner-confirmed reclamation is implemented.
        usage.bytes = original.size_bytes;
        usage.leases = usize::from(pending || matches!(record.status, Stage::Committed(_)));
    }
    Ok(usage)
}
