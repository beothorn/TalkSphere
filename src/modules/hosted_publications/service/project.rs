//! Value projections shared by the provider adapter and the requester client.
//!
//! A store carries its content, which quotes and assessments must not copy, so both sides derive the same body-free projection from a
//! request: operation, declared content size and retention. Every other operation projects to its complete request value.
use crate::modules::{
    hosted_publications::{
        codec::{self, Request},
        error::Error,
    },
    services::{
        self,
        value::{Budget, Value},
    },
};

/// The operation discriminator of a request value.
/// # Errors
/// `Invalid` when the value is not an object with a text `operation`.
pub(crate) fn operation(value: &Value) -> Result<&str, services::Error> {
    tracing::trace!("Reading a hosted publication operation discriminator");
    let Value::Object(fields) = value else {
        return Err(services::Error::Invalid);
    };
    match fields.iter().find(|(name, _)| name == "operation") {
        Some((_, Value::String(name))) => Ok(name),
        _ => Err(services::Error::Invalid),
    }
}

/// The assessment metadata of a complete request: the body-free projection of a store, otherwise the request itself.
/// # Errors
/// `Invalid`/`Limit` for a malformed or oversized request.
pub(crate) fn metadata(input: &Value, budget: Budget) -> Result<Value, services::Error> {
    tracing::trace!("Projecting hosted publication assessment metadata");
    input.bounded(budget)?;
    match codec::parse_request(input).map_err(map)? {
        Request::Store(store) => Ok(store_metadata(
            store.publication.metadata.content_size,
            store.retention_seconds,
        )),
        _ => Ok(input.clone()),
    }
}

/// The body-free store projection `{operation, contentSize, retentionSeconds}`.
pub(crate) fn store_metadata(content_size: u64, retention_seconds: u64) -> Value {
    tracing::trace!("Building the store assessment projection");
    Value::Object(vec![
        ("operation".into(), Value::String("store".into())),
        ("contentSize".into(), Value::Uint64(content_size)),
        ("retentionSeconds".into(), Value::Uint64(retention_seconds)),
    ])
}

/// A one-member factor object.
pub(crate) fn factor(name: &str, value: u64) -> Value {
    Value::Object(vec![(name.into(), Value::Uint64(value))])
}

/// A uint64 member of an object value.
/// # Errors
/// `Invalid` when absent or not a uint64.
pub(crate) fn number(value: &Value, name: &str) -> Result<u64, services::Error> {
    tracing::trace!("Reading a hosted publication uint64 field");
    let Value::Object(fields) = value else {
        return Err(services::Error::Invalid);
    };
    match fields.iter().find(|(n, _)| n == name) {
        Some((_, Value::Uint64(n))) => Ok(*n),
        _ => Err(services::Error::Invalid),
    }
}

/// Map a module error to Services.
pub(crate) fn map(error: Error) -> services::Error {
    tracing::trace!("Mapping a hosted publication error to Services");
    services::Error::from(error)
}
