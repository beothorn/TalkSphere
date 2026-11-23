//! Independent pinned Page schema recognition for Services registration; this catalogue installs no requester or provider code.
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

/// Locally installed Page version-one catalogue, bounded independently of definitions supplied by callers.
/// Source: [CR2SE section 10](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/pages/README.md#10-required-service-definition), embedded at build time.
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
        tracing::trace!("Loading independently pinned Page standard contract");
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
            "Constructing offering-bound Page definition without installing executable code"
        );
        if offering.service != "cr2se.page" || offering.version != 1 {
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
        tracing::trace!("Recognizing complete Page standard schemas before Services registration");
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
    tracing::trace!("Checking complete pinned Page definition identity and descriptor support");
    if value["service"] != "cr2se.page" || value.get("serviceVersion").and_then(integer) != Some(1)
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
    tracing::trace!("Comparing understood Page schema meaning with the pinned standard");
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
    tracing::trace!("Mapping bounded Page contract document validation failure");
    if error == board::Error::Limit {
        Error::Limit
    } else {
        Error::Invalid
    }
}
// Preserve the shared schema validator's bounded traversal failure rather than treating it as unsupported semantics.
fn schema_error(error: schema::Error) -> Error {
    tracing::trace!("Mapping Page standard schema format failure");
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
            return Err(io::Error::other("Page definition document limit"));
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
mod tests {
    use super::*;
    use crate::modules::board::offering::Board;
    use serde_json::json;

    // The provider test identity's ID text, as the offering's credit issuer.
    fn issuer() -> String {
        crate::modules::identity::text::format(crate::modules::identity::api::IdentityId([2; 32]))
    }

    // A syntactically valid offering built from the pinned acceptance terms.
    fn offering() -> Offering {
        let mut o: Value = serde_json::from_str(include_str!("fixtures/offering.json")).unwrap();
        o["creditIssuer"] = issuer().into();
        let board = json!({"version":1,"providedServices":[o],"wantedServices":[]});
        Board::parse(&serde_json::to_vec(&board).unwrap(), Limits::default())
            .unwrap()
            .offerings
            .remove("page")
            .unwrap()
    }

    // The pinned definition loads, builds an offering-bound definition and is recognized by Services as known.
    #[test]
    fn the_pinned_definition_is_recognized() {
        let catalogue = Catalogue::new(Limits::default()).unwrap();
        let definition = catalogue.definition(&offering()).unwrap();
        assert_eq!(definition.support, Support::Known);
        catalogue.validate(&definition).unwrap();
        let text: Value = serde_json::from_slice(definition.document.bytes()).unwrap();
        assert_eq!(text["id"], "page");
        assert_eq!(text["service"], "cr2se.page");
    }

    // Another service or version is unsupported, and an oversized id or description is a limit failure.
    #[test]
    fn other_families_and_oversized_fields_are_refused() {
        let catalogue = Catalogue::new(Limits::default()).unwrap();
        let mut other = offering();
        other.service = "cr2se.storage".into();
        assert_eq!(catalogue.definition(&other).err(), Some(Error::Unsupported));
        let mut newer = offering();
        newer.version = 2;
        assert_eq!(catalogue.definition(&newer).err(), Some(Error::Unsupported));
        let mut long = offering();
        long.description = "x".repeat(Limits::default().string_bytes + 1);
        assert_eq!(catalogue.definition(&long).err(), Some(Error::Limit));
    }

    // A definition whose understood schema differs (a changed field type) or that adds a required field is refused.
    #[test]
    fn altered_schemas_are_refused() {
        let catalogue = Catalogue::new(Limits::default()).unwrap();
        let good = catalogue.definition(&offering()).unwrap();
        let tamper = |edit: fn(&mut Value)| {
            let mut value: Value = serde_json::from_slice(good.document.bytes()).unwrap();
            edit(&mut value);
            let bytes = serde_json::to_vec(&value).unwrap();
            Definition::parse(&bytes, &offering(), Limits::default())
                .map_err(|_| Error::Invalid)
                .and_then(|d| catalogue.validate(&d))
        };
        assert!(
            tamper(|v| v["input"]["fields"]["maximumBytes"]["type"] = "string".into()).is_err()
        );
        assert!(tamper(|v| v["output"]["fields"]["price"]["type"] = "string".into()).is_err());
        assert!(
            tamper(|v| {
                v["input"]["fields"]["extra"] = json!({"type": "bytes", "description": "x"});
            })
            .is_err()
        );
        // An added optional field is a compatible extension and stays recognized.
        assert!(
            tamper(|v| {
                v["input"]["fields"]["extra"] =
                    json!({"type": "bytes", "description": "x", "optional": true});
            })
            .is_ok()
        );
        // Economics belong on the offering, never in the definition.
        assert!(tamper(|v| v["pricing"] = json!({})).is_err());
    }
}
