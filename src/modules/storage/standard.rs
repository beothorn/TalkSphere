//! Independent pinned Storage schema recognition for Services registration; this catalogue installs no requester or provider code.
use crate::modules::{
    board::{
        self,
        config::Limits,
        definition::Definition,
        document::Document,
        offering::{Offering, integer},
    },
    services::{
        Error,
        registry::Standards,
        schema::{self, Support},
    },
};
use serde_json::Value;
use std::io::{self, Write};

/// Locally installed Storage version-one catalogue, bounded independently of definitions supplied by callers.
/// Source: [CR2SE section 13](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#13-required-service-definition), embedded at build time.
pub struct Catalogue {
    template: Document,
    limits: Limits,
}
impl Catalogue {
    /// Load the pinned complete definition under local Board document budgets without registering or publishing any capability.
    /// # Errors
    /// Invalid rejects invalid limits/template shape; Limit rejects insufficient document budgets;
    /// Unsupported rejects an unrecognized embedded family/version or descriptor vocabulary.
    pub fn new(limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Loading independently pinned Storage standard contract");
        let template = Document::parse(include_bytes!("fixtures/definition.json"), limits)
            .map_err(document_error)?;
        base(template.value())?;
        Ok(Self { template, limits })
    }

    /// Construct the pinned complete logical definition bound to this offering's own ID and description.
    /// This checks schema identity only; callers separately validate economics, authority and actual handler availability.
    /// # Errors
    /// Unsupported rejects another service/version; Invalid malformed identity fields; Limit configured document bounds.
    pub fn definition(&self, offering: &Offering) -> Result<Definition, Error> {
        tracing::trace!(
            "Constructing offering-bound Storage definition without installing executable code"
        );
        if offering.service != "cr2se.storage" || offering.version != 1 {
            return Err(Error::Unsupported);
        }
        if offering.id.len() > self.limits.string_bytes
            || offering.description.len() > self.limits.string_bytes
        {
            return Err(Error::Limit);
        }
        let mut value = self.template.value().clone();
        value["id"] = Value::String(offering.id.clone());
        value["description"] = Value::String(offering.description.clone());
        let mut writer = Bounded {
            bytes: Vec::new(),
            maximum: self.limits.document_bytes,
            limited: false,
        };
        serde_json::to_writer(&mut writer, &value).map_err(|_| {
            if writer.limited {
                Error::Limit
            } else {
                Error::Invalid
            }
        })?;
        Definition::parse(&writer.bytes, offering, self.limits).map_err(document_error)
    }
}
impl Standards for Catalogue {
    // Reparse under independent budgets and compare known schema meaning rather than trusting a public support marker or peer prose.
    fn validate(&self, definition: &Definition) -> Result<(), Error> {
        tracing::trace!(
            "Recognizing complete Storage standard schemas before Services registration"
        );
        let checked =
            Document::parse(definition.document.bytes(), self.limits).map_err(document_error)?;
        let actual = checked.value();
        base(actual)?;
        if definition.support != Support::Known {
            return Err(Error::Unsupported);
        }
        let expected = self.template.value();
        for (a, b) in [
            (&actual["input"], &expected["input"]),
            (&actual["output"], &expected["output"]),
            (&actual["check"]["input"], &expected["check"]["input"]),
            (&actual["check"]["output"], &expected["check"]["output"]),
        ] {
            equivalent(a, b)?;
        }
        Ok(())
    }
}

// Require complete version-one identity, descriptions and all three bounded understood schemas, with economics kept on the offering.
fn base(value: &Value) -> Result<(), Error> {
    tracing::trace!("Checking complete pinned Storage definition identity and descriptor support");
    if value["service"] != "cr2se.storage"
        || value.get("serviceVersion").and_then(integer) != Some(1)
    {
        return Err(Error::Unsupported);
    }
    if !["id", "description"]
        .iter()
        .all(|name| value[*name].as_str().is_some_and(|text| !text.is_empty()))
        || ["creditIssuer", "price", "pricing", "checkPrice", "info"]
            .iter()
            .any(|name| value.get(*name).is_some())
    {
        return Err(Error::Invalid);
    }
    let supports = [
        schema::validate(value.get("input").ok_or(Error::Invalid)?).map_err(schema_error)?,
        schema::validate(value.get("output").ok_or(Error::Invalid)?).map_err(schema_error)?,
        schema::check(value.get("check").ok_or(Error::Invalid)?).map_err(schema_error)?,
    ];
    if supports.contains(&Support::Unknown) {
        return Err(Error::Unsupported);
    }
    Ok(())
}
// Compare the pinned known field types/optionality recursively; inert prose/annotations and supported optional extension fields do not change it.
fn equivalent(actual: &Value, expected: &Value) -> Result<(), Error> {
    tracing::trace!("Comparing understood Storage schema meaning with the pinned standard");
    if actual["type"] != expected["type"]
        || (actual["optional"] == true) != (expected["optional"] == true)
    {
        return Err(Error::Invalid);
    }
    if let Some(fields) = expected["fields"].as_object() {
        let actual_fields = actual["fields"].as_object().ok_or(Error::Invalid)?;
        for (name, descriptor) in fields {
            equivalent(actual_fields.get(name).ok_or(Error::Invalid)?, descriptor)?;
        }
        if actual_fields
            .iter()
            .any(|(name, descriptor)| !fields.contains_key(name) && descriptor["optional"] != true)
        {
            return Err(Error::Unsupported);
        }
    }
    Ok(())
}
// Preserve configured document bounds separately from malformed local/public input without exposing another module's errors.
fn document_error(error: board::Error) -> Error {
    tracing::trace!("Mapping bounded Storage contract document validation failure");
    if error == board::Error::Limit {
        Error::Limit
    } else {
        Error::Invalid
    }
}
// Preserve the shared schema validator's bounded traversal failure rather than treating it as unsupported semantics.
fn schema_error(error: schema::Error) -> Error {
    tracing::trace!("Mapping Storage standard schema format failure");
    match error {
        schema::Error::Invalid => Error::Invalid,
        schema::Error::Limit => Error::Limit,
    }
}
struct Bounded {
    bytes: Vec<u8>,
    maximum: usize,
    limited: bool,
}
impl Write for Bounded {
    // Stop before appending beyond the document bound, including JSON escape expansion.
    // Trace at the definition boundary: serializers can call this once per escaped byte.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.maximum)
        {
            self.limited = true;
            return Err(io::Error::other("Storage definition document limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    // This in-memory writer has no external flush or durability effects.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[cfg(test)]
#[path = "tests/standard.rs"]
mod tests;
