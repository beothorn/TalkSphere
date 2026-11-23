//! Value projections shared by the provider adapter and the requester client.
//!
//! A store carries its content, which quotes and assessments must not copy, so both sides derive the same body-free projection from a
//! request: operation, declared content size and retention. Every other operation projects to its complete request value.
use crate::modules::{
    hosted_publications::error::Error,
    services::{self, value::Value},
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

/// Map a module error to Services.
pub(crate) fn map(error: Error) -> services::Error {
    tracing::trace!("Mapping a hosted publication error to Services");
    services::Error::from(error)
}
