//! Private bounded result retention shared by accepted metadata mutations under recovered serialized admission.
use super::{
    effect::Effect,
    journal::Journal,
    quota::{Quota, Usage},
};
use crate::modules::services::{
    self, Error,
    contract::OperationKey,
    handler::{Blob, Completion},
    value::Value,
};

// Encode a complete proposal, reserve its original hold before durable staging, and preserve uncertainty rather than implying abort.
pub(super) fn retain(
    journal: &Journal,
    quota: &Quota,
    key: OperationKey,
    binding: [u8; 32],
    output: Value,
    price: u64,
    effect: Effect,
) -> Result<Completion, Error> {
    tracing::trace!("Retaining bounded Storage mutation output before a coordinator decision");
    let limits = journal.image_limits();
    let encoded = services::codec::encode(&output, limits.values, limits.bytes)?;
    let effect = effect.encode(limits)?;
    let usage = Usage {
        operations: 1,
        result_bytes: u64::try_from(encoded.len())
            .map_err(|_| Error::Limit)?
            .checked_mul(2)
            .ok_or(Error::Limit)?,
        ..Usage::default()
    };
    let completion = Completion {
        output,
        result: Blob::new(encoded, limits.bytes)?,
        price,
        effect: Blob::new(effect, limits.bytes)?,
    };
    quota.reserve(key, usage)?;
    match journal.stage(key, binding, completion) {
        Ok(result) => Ok(result),
        Err(Error::Unknown) => Err(Error::Unknown),
        Err(error) => {
            quota
                .transition(key, Usage::default())
                .map_err(|_| Error::Unknown)?;
            Err(error)
        }
    }
}
