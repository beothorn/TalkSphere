//! Narrow owned-body family fixture verifies exact content before any provider work.
use super::family::Echo;
use crate::modules::{
    board::{definition::Definition, offering::Offering},
    services::{
        Error,
        handler::*,
        validation::Semantics,
        value::{Budget, Value},
    },
};

pub(in crate::modules::services) struct Body(pub Echo, Definition);
impl Body {
    // The fixture's accepted metadata and expected content are literal complete three-byte input.
    pub(in crate::modules::services) fn new() -> Self {
        let echo = Echo::new();
        let mut definition = echo.definition().clone();
        let mut value = definition.document.value().clone();
        value["input"] = serde_json::json!({"type":"bytes"});
        definition.document = crate::modules::board::document::Document::parse(
            value.to_string().as_bytes(),
            Default::default(),
        )
        .unwrap();
        Self(echo, definition)
    }
}
impl Semantics for Body {
    // Bytes are logical input and text is the staged output; detailed body equality is checked separately.
    fn validate(&self, _: &serde_json::Value, value: &Value) -> Result<(), Error> {
        if matches!(value, Value::Bytes(_) | Value::String(_)) {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
    // No numeric extension is installed.
    fn nonfinite(&self) -> bool {
        false
    }
}
impl Requester for Body {
    // Exact literal definition changes only the input vocabulary of the custom echo contract.
    fn definition(&self) -> &Definition {
        &self.1
    }
    // The fixed seven-credit price remains independently understood.
    fn terms(&self, offering: &Offering) -> Result<(), Error> {
        self.0.terms(offering)
    }
    // Complete byte length and content hash must agree with assessed metadata before dispatch.
    fn validate_input(&self, metadata: &Value, input: &Value, budget: Budget) -> Result<(), Error> {
        use sha2::{Digest, Sha256};
        input.bounded(budget)?;
        let (Value::Bytes(expected), Value::Bytes(bytes)) = (metadata, input) else {
            return Err(Error::Invalid);
        };
        if bytes.len() == expected.len() && Sha256::digest(bytes) == Sha256::digest(expected) {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
    // No local original evidence proves a property in this narrow input-only fixture.
    fn verify(&self, _: &Value, _: &Blob, _: &Value, _: Option<&Value>) -> Result<Check, Error> {
        Ok(Check::Inconclusive)
    }
}
