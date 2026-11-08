//! Narrow standard requester fixtures; no family provider, index, feed or language runtime is installed.
use crate::modules::{
    board::{definition::Definition, offering::Offering},
    services::{
        Error,
        handler::{Blob, Check, Invocation, Requester},
        pricing::Price,
        registry::Standards,
        validation::Semantics,
        value::Value,
    },
};

pub(in crate::modules::services) struct Catalogue;
impl Standards for Catalogue {
    // Standard identity is established by an independent pinned definition, never a name alone.
    fn validate(&self, definition: &Definition) -> Result<(), Error> {
        let matches = [
            include_str!("computation-definition.json"),
            include_str!("discovery-definition.json"),
            include_str!("feeds-definition.json"),
        ]
        .iter()
        .any(|raw| {
            definition.document.value() == &serde_json::from_str::<serde_json::Value>(raw).unwrap()
        });
        if matches {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
    }
}
pub(in crate::modules::services) struct Contract(pub Definition);
// Read one exact logical field for the test-owned immutable Contract contract.
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    let Value::Object(fields) = value else {
        return Err(Error::Invalid);
    };
    fields
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .ok_or(Error::Invalid)
}
impl Semantics for Contract {
    // Only fixture input/result discriminator combinations are accepted, independently of shape validation.
    fn validate(&self, _: &serde_json::Value, value: &Value) -> Result<(), Error> {
        if self.0.document.value()["service"] != "cr2se.computation" {
            let discovery = self.0.document.value()["service"] == "cr2se.discovery";
            if field(value, "operation")?
                != &Value::String(if discovery { "listIdentities" } else { "read" }.into())
            {
                return Err(Error::Invalid);
            }
            if field(value, "price").is_ok() {
                if field(value, if discovery { "identities" } else { "messages" })?
                    != &Value::Array(vec![])
                    || field(value, "more")? != &Value::Bool(false)
                {
                    return Err(Error::Invalid);
                }
            } else if field(value, "limit")? != &Value::Uint32(1) {
                return Err(Error::Invalid);
            }
            return Ok(());
        }
        if let Ok(Value::String(outcome)) = field(value, "outcome") {
            let has_result = field(value, "result").is_ok();
            let has_diagnostic = field(value, "diagnostic").is_ok();
            match outcome.as_str() {
                "result" if has_result && !has_diagnostic => Ok(()),
                "executionError" if !has_result => Ok(()),
                "timeLimitReached"
                    if !has_result
                        && !has_diagnostic
                        && field(value, "billedMilliseconds")? == &Value::Uint64(60000) =>
                {
                    Ok(())
                }
                _ => Err(Error::Invalid),
            }
        } else if field(value, "language")? == &Value::String("javascript".into())
            && field(value, "languageVersion")? == &Value::String("ECMAScript 2024".into())
            && field(value, "maxExecutionMilliseconds")? == &Value::Uint64(60000)
        {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
    // Standard Contract strings and integer fields grant no nonfinite permission.
    fn nonfinite(&self) -> bool {
        false
    }
}
impl Requester for Contract {
    // The requester retains the literal upstream definition even with no local provider.
    fn definition(&self) -> &Definition {
        &self.0
    }
    // Only the pinned offering rate is understood by this narrow contract fixture.
    fn terms(&self, offering: &Offering) -> Result<(), Error> {
        if offering.service != "cr2se.computation" {
            let expected: serde_json::Value =
                serde_json::from_str(if offering.service == "cr2se.discovery" {
                    include_str!("discovery-offering.json")
                } else {
                    include_str!("feeds-offering.json")
                })
                .unwrap();
            return if offering.source["pricing"] == expected["pricing"] {
                Ok(())
            } else {
                Err(Error::Unsupported)
            };
        }
        if offering.source["pricing"]
            == serde_json::json!({"model":"cr2se.computation.v1","minimumPrice":1,"executionTimeFactor":{"numerator":1,"denominator":1000}})
        {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
    }
    // Independent literal terms reproduce the upstream maximum sixty, final two example.
    fn pricing(&self, offering: &Offering, _: &Value) -> Result<Price, Error> {
        self.terms(offering)?;
        if offering.service != "cr2se.computation" {
            return Ok(Price::Fixed(if offering.service == "cr2se.discovery" {
                2
            } else {
                1
            }));
        }
        Ok(Price::Computation {
            minimum: 1,
            numerator: 1,
            denominator: 1000,
            milliseconds: 60000,
        })
    }
    // Price reported inside the typed completion must independently agree with its billed time.
    fn validate_result(
        &self,
        invocation: &Invocation,
        _: &Value,
        output: &Value,
    ) -> Result<(), Error> {
        if invocation.offering.service != "cr2se.computation" {
            return if field(output, "price")? == &Value::Uint64(invocation.maximum) {
                Ok(())
            } else {
                Err(Error::Invalid)
            };
        }
        let Value::Uint64(billed) = field(output, "billedMilliseconds")? else {
            return Err(Error::Invalid);
        };
        let price = invocation
            .pricing
            .final_price("cr2se.computation", 1, Some(*billed))?;
        if field(output, "price")? == &Value::Uint64(price) {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
    // No execution engine is installed, so local re-execution is explicitly inconclusive.
    fn verify(&self, _: &Value, _: &Blob, _: &Value, _: Option<&Value>) -> Result<Check, Error> {
        Ok(Check::Inconclusive)
    }
}
