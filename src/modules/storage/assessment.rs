//! Store metadata assessment before body receipt; projections contain known dimensions, never placeholder content.
use super::{pricing, terms::Terms};
use crate::modules::{
    board::offering::Offering,
    services::{
        Error,
        handler::Assessment,
        input::Input,
        pricing::Price,
        value::{Budget, Value},
    },
};

/// Validated store assessment and exact work price; no execution, lease or financial reservation is created.
pub struct Store {
    /// Bounded local metadata, empty provider factors and full declared logical input byte bound.
    pub assessment: Assessment,
    /// Exact service work price before separately authorized issuer fees.
    pub price: u64,
}
#[derive(Clone, Copy)]
struct Dimensions {
    size: u64,
    seconds: u64,
}
enum Object<'a> {
    Complete(&'a [(String, Value)]),
    Described(&'a [(String, Input)]),
}
enum Field<'a> {
    Value(&'a Value),
    Description(&'a Input),
}

/// Assess a complete or unresolved store input before upload admission and enforce an explicit work-price bound.
/// Hash/data references are validated by declared exact lengths without inventing their content.
/// # Errors
/// Invalid rejects malformed fields, forbidden known fields or type/length mismatch; Limit rejects resource/term bounds;
/// Insufficient rejects an exact work price above maximum. Authentication and total fee authority remain with Services.
pub fn store(
    input: &Input,
    offering: &Offering,
    budget: Budget,
    maximum: u64,
) -> Result<Store, Error> {
    tracing::trace!("Assessing exact Storage store dimensions before body execution");
    let input_bytes = u64::try_from(input.bounded(budget)?).map_err(|_| Error::Limit)?;
    let terms = Terms::parse(offering)?;
    let object = match input {
        Input::Complete(Value::Object(f)) => Object::Complete(f),
        Input::Object(f) => Object::Described(f),
        _ => return Err(Error::Invalid),
    };
    let dimensions = dimensions(&object, terms)?;
    let price = pricing::store(
        dimensions.size,
        dimensions.seconds,
        terms.upload,
        terms.retention,
        terms.minimum_price,
    )?;
    if price > maximum {
        return Err(Error::Insufficient);
    }
    Ok(Store {
        assessment: Assessment {
            metadata: projection(dimensions),
            factors: Value::Object(vec![]),
            input_bytes,
            pin: None,
        },
        price,
    })
}

/// Assess through a recovered Linux owner and check current capacity before accepting a bounded upload.
/// Capacity is a provisional check, not a durable lease or reservation; execution rechecks/reserves before writes.
/// # Errors
/// Preserves metadata/price failures and rejects closed recovery gates, existing keys or exhausted capacity.
#[cfg(target_os = "linux")]
pub fn admitted_store(
    gate: &super::recovery::Gate,
    key: crate::modules::services::contract::OperationKey,
    input: &Input,
    offering: &Offering,
    budget: Budget,
    maximum: u64,
) -> Result<Store, Error> {
    tracing::trace!("Checking recovered Storage store capacity before upload admission");
    let assessed = store(input, offering, budget, maximum)?;
    let Value::Object(fields) = &assessed.assessment.metadata else {
        return Err(Error::Invalid);
    };
    let size = number(&Object::Complete(fields), "sizeBytes")?;
    gate.admit(|_, quota| {
        quota.probe(
            key,
            super::quota::Usage {
                bytes: size,
                leases: 1,
                operations: 1,
                ..super::quota::Usage::default()
            },
        )?;
        Ok(assessed)
    })
}

/// Derive the same bounded local pricing metadata directly from authorized requester input without cloning payload bytes.
/// # Errors
/// Invalid rejects malformed complete inputs; Limit rejects budgets and current offering limits.
pub fn requester_metadata(
    input: &Value,
    offering: &Offering,
    budget: Budget,
) -> Result<Value, Error> {
    tracing::trace!("Projecting requester-owned Storage store dimensions");
    input.bounded(budget)?;
    let Value::Object(fields) = input else {
        return Err(Error::Invalid);
    };
    Ok(projection(dimensions(
        &Object::Complete(fields),
        Terms::parse(offering)?,
    )?))
}

/// Recalculate store pricing from the validated local projection and empty provider-only factors.
/// # Errors
/// Invalid rejects unknown/malformed projections or invented provider factors; offering/price failures propagate.
pub fn store_price(offering: &Offering, metadata: &Value, factors: &Value) -> Result<Price, Error> {
    tracing::trace!("Recalculating Storage store quote from independently known metadata");
    metadata.bounded(Budget {
        depth: 1,
        nodes: 4,
        bytes: 64,
    })?;
    let Value::Object(fields) = metadata else {
        return Err(Error::Invalid);
    };
    if fields.len() != 3 || !matches!(factors,Value::Object(f) if f.is_empty()) {
        return Err(Error::Invalid);
    }
    let object = Object::Complete(fields);
    require_store(&object)?;
    let dimensions = Dimensions {
        size: number(&object, "sizeBytes")?,
        seconds: number(&object, "retentionSeconds")?,
    };
    let terms = Terms::parse(offering)?;
    limits(dimensions, terms)?;
    Ok(Price::Fixed(pricing::store(
        dimensions.size,
        dimensions.seconds,
        terms.upload,
        terms.retention,
        terms.minimum_price,
    )?))
}

// Inspect only required store fields; ignored extensions remain subject to the original complete input budgets.
fn dimensions(object: &Object<'_>, terms: Terms) -> Result<Dimensions, Error> {
    tracing::trace!("Validating known Storage metadata dimensions and byte references");
    require_store(object)?;
    for forbidden in [
        "leaseId",
        "offset",
        "length",
        "expectedExpiresAt",
        "additionalSeconds",
    ] {
        if field(object, forbidden).is_ok() {
            return Err(Error::Invalid);
        }
    }
    let dimensions = Dimensions {
        size: number(object, "sizeBytes")?,
        seconds: number(object, "retentionSeconds")?,
    };
    limits(dimensions, terms)?;
    if byte_length(field(object, "contentHash")?)? != 32
        || byte_length(field(object, "data")?)? != dimensions.size
    {
        return Err(Error::Invalid);
    }
    Ok(dimensions)
}
// Bounds use exact declared uint64 values; zero has no Storage v1 utility or retention meaning.
fn limits(d: Dimensions, t: Terms) -> Result<(), Error> {
    tracing::trace!("Checking Storage store dimensions against current accepted limits");
    if d.size == 0 || d.seconds == 0 {
        return Err(Error::Invalid);
    }
    if d.size > t.maximum_bytes || d.seconds < t.minimum_seconds || d.seconds > t.maximum_seconds {
        return Err(Error::Limit);
    }
    Ok(())
}
// Keep the local projection independent of body representation and actual yet-unreceived hash/data content.
fn projection(d: Dimensions) -> Value {
    tracing::trace!("Creating local Storage store pricing metadata without payload placeholders");
    Value::Object(vec![
        ("operation".into(), Value::String("store".into())),
        ("sizeBytes".into(), Value::Uint64(d.size)),
        ("retentionSeconds".into(), Value::Uint64(d.seconds)),
    ])
}
// The public check mode and follow-up operation discriminants cannot enter store assessment.
fn require_store(object: &Object<'_>) -> Result<(), Error> {
    tracing::trace!("Checking Storage store operation discriminant before assessment");
    match field(object, "operation")? {
        Field::Value(Value::String(s)) if s == "store" => Ok(()),
        _ => Err(Error::Invalid),
    }
}
// Borrow exact known values from either complete owned input or unresolved input descriptions.
fn field<'a>(object: &Object<'a>, name: &str) -> Result<Field<'a>, Error> {
    tracing::trace!("Reading known Storage store metadata field");
    match object {
        Object::Complete(f) => f
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| Field::Value(v))
            .ok_or(Error::Invalid),
        Object::Described(f) => f
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| match v {
                Input::Complete(value) => Field::Value(value),
                _ => Field::Description(v),
            })
            .ok_or(Error::Invalid),
    }
}
// A number must be complete exact metadata; a byte descriptor cannot stand in for a numeric value.
fn number(object: &Object<'_>, name: &str) -> Result<u64, Error> {
    tracing::trace!("Reading exact Storage store metadata integer");
    match field(object, name)? {
        Field::Value(Value::Uint64(n)) => Ok(*n),
        _ => Err(Error::Invalid),
    }
}
// Both complete bytes and explicit binary body references expose length without allocating placeholder bytes.
fn byte_length(field: Field<'_>) -> Result<u64, Error> {
    tracing::trace!("Checking Storage byte metadata length without materializing bodies");
    match field {
        Field::Value(Value::Bytes(b)) => u64::try_from(b.len()).map_err(|_| Error::Limit),
        Field::Description(Input::Body {
            length,
            text: false,
        }) => Ok(*length),
        _ => Err(Error::Invalid),
    }
}
#[cfg(test)]
#[path = "tests/assessment.rs"]
mod tests;
