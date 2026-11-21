//! Stage a paid immutable upload and finalize its lease only at the original Services decision.
use super::{
    contract::{Crypto, Lease, State, Time},
    effect::Effect,
    image::{Limits, Stage},
    quota::Usage,
    recovery::Gate,
};
use crate::modules::{
    identity::api::IdentityId,
    services::{
        self, Error,
        contract::OperationKey,
        handler::{Blob, Completion, Invocation},
        pricing::Price,
        value::{Budget, Value},
    },
};

/// Stage exact upload bytes and fixed lease fields under the accepted operation key.
/// # Errors
/// Invalid rejects malformed, inconsistent or corrupt uploads. Limit rejects exhausted resources or
/// bounded ID allocation. Unknown preserves resources until recovery reconciles possible writes.
pub fn stage(
    gate: &Gate,
    invocation: &Invocation,
    input: &Value,
    crypto: &dyn Crypto,
    budget: Budget,
) -> Result<Completion, Error> {
    tracing::trace!("Staging durable Storage bytes and a timestamp-free lease proposal");
    super::binding::check(invocation)?;
    let (mut lease, data) = describe(invocation, input, budget)?;
    if crypto.hash(data)? != lease.content_hash {
        return Err(Error::Invalid);
    }
    gate.admit(|journal, quota| {
        let limits = journal.image_limits();
        let binding = super::binding::retained(invocation, input, limits)?;
        if let Some(original) =
            journal.original_result(invocation.key, binding, limits.values, limits.bytes)?
        {
            if let Effect::CreateTemplate(lease) =
                Effect::decode(original.effect.bytes(), invocation.provider, limits)?
            {
                gate.byte_store().reopen(lease.id, lease.size_bytes)?;
            }
            return Ok(original);
        }
        let image = journal.snapshot()?;
        if image.provider != invocation.provider {
            return Err(Error::Denied);
        }
        let bytes = gate.byte_store();
        let maximum_entries = limits
            .leases
            .checked_add(limits.operations)
            .ok_or(Error::Limit)?;
        let mut known = bytes
            .inventory(maximum_entries)?
            .into_iter()
            .map(|entry| entry.id)
            .collect::<std::collections::BTreeSet<_>>();
        known.extend(image.leases.iter().map(|lease| lease.id));
        for record in &image.operations {
            if let Some(original) =
                Effect::decode(record.completion.effect.bytes(), image.provider, limits)?
                    .created_lease()
            {
                known.insert(original.id);
            }
        }
        const ID_ATTEMPTS: usize = 16;
        let mut selected = None;
        for _ in 0..ID_ATTEMPTS {
            let id = crypto.lease_id()?;
            if !known.contains(&id) {
                selected = Some(id);
                break;
            }
        }
        lease.id = selected.ok_or(Error::Limit)?;
        let template = completion(Effect::CreateTemplate(lease.clone()), limits)?;
        let result_bytes = template
            .result
            .bytes()
            .len()
            .checked_mul(2)
            .ok_or(Error::Limit)?;
        quota.reserve(
            invocation.key,
            Usage {
                bytes: lease.size_bytes,
                leases: 1,
                operations: 1,
                result_bytes: u64::try_from(result_bytes).map_err(|_| Error::Limit)?,
            },
        )?;
        // Persist the upload intent before any object path exists, so a crash cannot create an unowned file.
        let template = match journal.stage(invocation.key, binding, template) {
            Ok(template) => template,
            Err(Error::Unknown) => return Err(Error::Unknown),
            Err(error) => {
                quota
                    .transition(invocation.key, Usage::default())
                    .map_err(|_| Error::Unknown)?;
                return Err(error);
            }
        };
        let mut upload = bytes.begin(lease.id, lease.size_bytes, lease.size_bytes)?;
        upload.append(0, data)?;
        drop(upload.finish(lease.content_hash)?);
        Ok(template)
    })
}

/// Validate a recovered template against original authorized input and terms without reading a clock.
/// # Errors
/// Invalid rejects any changed fixed field, input dimension or output encoding.
pub fn validate_template(
    invocation: &Invocation,
    input: &Value,
    template: &Completion,
    limits: Limits,
) -> Result<(), Error> {
    tracing::trace!("Validating fixed Storage template fields against original input");
    let (mut expected, _) = describe(invocation, input, limits.values)?;
    let Effect::CreateTemplate(actual) =
        Effect::decode(template.effect.bytes(), invocation.provider, limits)?
    else {
        return Err(Error::Invalid);
    };
    expected.id = actual.id;
    compare(
        template,
        &completion(Effect::CreateTemplate(expected), limits)?,
    )
}

/// Sampled time fills only the two authorized timestamp fields and binds the retention observation.
/// # Errors
/// Unknown or Conflict rejects missing prepared state or wrong token. Limit rejects expiry overflow.
pub fn finalize(
    gate: &Gate,
    key: OperationKey,
    token: &Blob,
    template: &Completion,
    now: Time,
    reboot_elapsed: Option<u64>,
) -> Result<Completion, Error> {
    tracing::trace!("Finalizing prepared Storage lease for coordinator decision selection");
    let (image, limits) = gate.snapshot()?;
    let record = image
        .operations
        .iter()
        .find(|r| r.key == key)
        .ok_or(Error::Unknown)?;
    if !matches!(&record.status, Stage::Prepared(original) if original == token) {
        return Err(Error::Conflict);
    }
    compare(template, &record.completion)?;
    let Effect::CreateTemplate(mut lease) =
        Effect::decode(template.effect.bytes(), image.provider, limits)?
    else {
        return Err(Error::Invalid);
    };
    let anchor = image.anchor.advance(now, reboot_elapsed)?;
    lease.expires_at = super::retention::expiration(now.unix_seconds, lease.expires_at)?;
    lease.accepted_at = now.unix_seconds;
    completion(Effect::CreateAt { lease, anchor }, limits)
}

// Check both the prepared and final result bindings before applying the durable decision.
pub(super) fn validate_finalized(
    provider: IdentityId,
    limits: Limits,
    template: &Completion,
    finalized: &Completion,
) -> Result<Effect, Error> {
    tracing::trace!("Checking finalized Storage lease against immutable preparation");
    let Effect::CreateTemplate(mut lease) =
        Effect::decode(template.effect.bytes(), provider, limits)?
    else {
        return Err(Error::Invalid);
    };
    let Effect::CreateAt { anchor, .. } =
        Effect::decode(finalized.effect.bytes(), provider, limits)?
    else {
        return Err(Error::Invalid);
    };
    compare(
        template,
        &completion(Effect::CreateTemplate(lease.clone()), limits)?,
    )?;
    lease.accepted_at = anchor.observation.unix_seconds;
    lease.expires_at = super::retention::expiration(lease.accepted_at, lease.expires_at)?;
    let effect = Effect::CreateAt { lease, anchor };
    compare(finalized, &completion(effect.clone(), limits)?)?;
    Ok(effect)
}

// Derive fixed template fields directly from the exact accepted contract and complete upload.
fn describe<'a>(
    invocation: &Invocation,
    input: &'a Value,
    budget: Budget,
) -> Result<(Lease, &'a [u8]), Error> {
    tracing::trace!("Deriving exact Storage upload terms and byte binding");
    if super::operation::input(input, budget)? != super::operation::Operation::Store
        || !matches!(&invocation.factors, Value::Object(fields) if fields.is_empty())
    {
        return Err(Error::Invalid);
    }
    let metadata = super::assessment::requester_metadata(input, &invocation.offering, budget)?;
    let Price::Fixed(price) =
        super::assessment::store_price(&invocation.offering, &metadata, &invocation.factors)?
    else {
        return Err(Error::Invalid);
    };
    if invocation.pricing != Price::Fixed(price) || invocation.maximum != price {
        return Err(Error::Invalid);
    }
    let Value::Object(fields) = input else {
        return Err(Error::Invalid);
    };
    let get_bytes = |name| match fields.iter().find(|(field, _)| field == name) {
        Some((_, Value::Bytes(bytes))) => Ok(bytes.as_slice()),
        _ => Err(Error::Invalid),
    };
    let lease = Lease {
        id: [0; 32],
        requester: invocation.requester,
        provider: invocation.provider,
        accepted_terms: invocation.accepted.clone(),
        size_bytes: super::renewal::number(input, "sizeBytes")?,
        content_hash: get_bytes("contentHash")?
            .try_into()
            .map_err(|_| Error::Invalid)?,
        accepted_at: 0,
        expires_at: super::renewal::number(input, "retentionSeconds")?,
        store_price: price,
        state: State::Active,
    };
    Ok((lease, get_bytes("data")?))
}

// Reserve final shape and size up front. The local template has no authority to publish these placeholders.
fn completion(effect: Effect, limits: Limits) -> Result<Completion, Error> {
    tracing::trace!("Encoding exact bounded Storage creation completion");
    let lease = effect.created_lease().ok_or(Error::Invalid)?;
    let output = Value::Object(vec![
        ("operation".into(), Value::String("store".into())),
        ("leaseId".into(), Value::Bytes(lease.id.to_vec())),
        ("sizeBytes".into(), Value::Uint64(lease.size_bytes)),
        (
            "contentHash".into(),
            Value::Bytes(lease.content_hash.to_vec()),
        ),
        ("acceptedAt".into(), Value::Uint64(lease.accepted_at)),
        ("expiresAt".into(), Value::Uint64(lease.expires_at)),
        ("price".into(), Value::Uint64(lease.store_price)),
    ]);
    Ok(Completion {
        result: Blob::new(
            services::codec::encode(&output, limits.values, limits.bytes)?,
            limits.bytes,
        )?,
        output,
        price: lease.store_price,
        effect: Blob::new(effect.encode(limits)?, limits.bytes)?,
    })
}

// Reject a changed output, encoded result, owner effect or prepared charge.
fn compare(a: &Completion, b: &Completion) -> Result<(), Error> {
    tracing::trace!("Verifying the exact Storage creation completion");
    if a.output != b.output || a.result != b.result || a.effect != b.effect || a.price != b.price {
        return Err(Error::Conflict);
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/store.rs"]
mod tests;
