//! Value projections shared by the provider adapter and the requester client.
//!
//! A store carries its payload, which quotes and assessments must not copy, so both sides derive the same body-free projection
//! from a request: operation, declared payload size and retention. Every other operation projects to its complete request value.
use crate::modules::{
    messaging::{
        codec::{self, Request},
        error::Error,
    },
    services::{
        self,
        value::{Budget, Value},
    },
};

/// The operation name of a request or output value.
pub(super) fn operation(value: &Value) -> Result<&str, services::Error> {
    tracing::trace!("Reading a Messaging operation discriminator");
    let Value::Object(fields) = value else {
        return Err(services::Error::Invalid);
    };
    match fields.iter().find(|(name, _)| name == "operation") {
        Some((_, Value::String(name))) => Ok(name),
        _ => Err(services::Error::Invalid),
    }
}

/// The body-free assessment metadata of a complete request value.
/// # Errors
/// `Invalid` for a malformed request.
pub(super) fn metadata(input: &Value, budget: Budget) -> Result<Value, services::Error> {
    tracing::trace!("Projecting Messaging assessment metadata");
    input.bounded(budget)?;
    match codec::parse_request(input).map_err(map)? {
        Request::Store(store) => Ok(store_metadata(
            store.message.metadata.payload_size,
            store.retention_seconds,
        )),
        _ => Ok(input.clone()),
    }
}

/// The projection of a store: operation, declared payload size and retention.
pub(super) fn store_metadata(payload_size: u64, retention_seconds: u64) -> Value {
    tracing::trace!("Building the store assessment projection");
    Value::Object(vec![
        ("operation".into(), Value::String("store".into())),
        ("payloadSize".into(), Value::Uint64(payload_size)),
        ("retentionSeconds".into(), Value::Uint64(retention_seconds)),
    ])
}

/// A single-field provider factor object, such as `payloadSize` or `releaseCredits`.
pub(super) fn factor(name: &str, value: u64) -> Value {
    Value::Object(vec![(name.into(), Value::Uint64(value))])
}

/// Read one exact uint64 field of an object value.
pub(super) fn number(value: &Value, name: &str) -> Result<u64, services::Error> {
    tracing::trace!("Reading a Messaging uint64 field");
    let Value::Object(fields) = value else {
        return Err(services::Error::Invalid);
    };
    match fields.iter().find(|(n, _)| n == name) {
        Some((_, Value::Uint64(n))) => Ok(*n),
        _ => Err(services::Error::Invalid),
    }
}

/// Map a Messaging failure to the Services category with the same meaning.
pub(super) fn map(error: Error) -> services::Error {
    tracing::trace!("Mapping a Messaging error to Services");
    services::Error::from(error)
}
