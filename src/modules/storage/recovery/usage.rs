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
    retained: &BTreeSet<[u8; 32]>,
) -> Result<Quota, Error> {
    tracing::trace!("Reconstructing original Storage resource holds before admission");
    let quota = Quota::new(limits)?;
    let mut committed = BTreeSet::new();
    for record in &image.operations {
        if matches!(record.status, Stage::Staged | Stage::Prepared(_)) {
            return Err(Error::Unknown);
        }
        let usage = record_usage(image, record, image_limits, retained)?;
        if let Some(original) = Effect::decode(
            record.completion.effect.bytes(),
            image.provider,
            image_limits,
        )?
        .created_lease()
            && matches!(record.status, Stage::Committed(_))
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
    retained: &BTreeSet<[u8; 32]>,
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
    if let Some(original) = Effect::decode(
        record.completion.effect.bytes(),
        image.provider,
        image_limits,
    )?
    .created_lease()
    {
        // Only physically present or descriptor-retained object bytes consume byte capacity; ID and result history remains.
        usage.bytes = if retained.contains(&original.id) {
            original.size_bytes
        } else {
            0
        };
        usage.leases = usize::from(pending || matches!(record.status, Stage::Committed(_)));
    }
    Ok(usage)
}

// Retain directory-backed and descriptor-only object IDs under finite owner inventory bounds.
pub(super) fn objects(
    bytes: &super::super::bytes::Store,
    image: &Image,
    limits: super::super::image::Limits,
) -> Result<BTreeSet<[u8; 32]>, Error> {
    tracing::trace!("Accounting for directory-backed and pinned Storage object bytes");
    let maximum = limits
        .leases
        .checked_add(limits.operations)
        .ok_or(Error::Limit)?;
    let mut retained = bytes
        .inventory(maximum)?
        .into_iter()
        .map(|entry| entry.id)
        .collect::<BTreeSet<_>>();
    for record in &image.operations {
        if let Some(original) =
            Effect::decode(record.completion.effect.bytes(), image.provider, limits)?
                .created_lease()
            && bytes.has_pins(original.id)?
        {
            retained.insert(original.id);
        }
    }
    Ok(retained)
}
