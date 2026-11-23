//! The Page requester contract and economic rules, installed for both the visitor and the publisher role.
//!
//! Services validates quotes, inputs and results through one installed requester per contract. The price is the offering's fixed price; a result must belong to
//! its request and pass the local check before it can authorize settlement. The check is local and included: it sends nothing to the publisher and costs nothing.
//! Dependencies: the terms, the codec, the local check and the Services contracts.
use super::{
    check::{self, Evidence, Outcome, Returned},
    codec,
    standard::Catalogue,
    terms::Terms,
};
use crate::modules::{
    board::{definition::Definition, offering::Offering},
    services::{
        Error,
        handler::{Blob, Check, Invocation, Requester},
        validation::Semantics,
        value::Value,
    },
};

/// The requester implementation; construction registers nothing and publishes nothing.
pub struct Client {
    definition: Definition,
    values: crate::modules::services::value::Budget,
}

impl Client {
    /// Build the client for a definition the catalogue recognizes as the pinned Page contract.
    /// # Errors
    /// `Invalid` for unusable value bounds, `Unsupported` or `Invalid` when the catalogue does not recognize the definition.
    pub fn new(
        definition: Definition,
        catalogue: &Catalogue,
        values: crate::modules::services::value::Budget,
    ) -> Result<Self, Error> {
        tracing::trace!("Constructing the Page requester client");
        if values.depth == 0 || values.depth > 31 || values.nodes == 0 || values.bytes == 0 {
            return Err(Error::Invalid);
        }
        use crate::modules::services::registry::Standards;
        catalogue.validate(&definition)?;
        Ok(Self { definition, values })
    }
}

impl Semantics for Client {
    // Apply the pinned operation rules to this installed contract's own descriptors only.
    fn validate(&self, descriptor: &serde_json::Value, value: &Value) -> Result<(), Error> {
        tracing::trace!("Applying Page contract semantics");
        let definition = self.definition.document.value();
        value.bounded(self.values)?;
        if descriptor == &definition["input"] {
            codec::parse_request(value).map(|_| ()).map_err(Error::from)
        } else if descriptor == &definition["output"] {
            codec::parse_output(value).map(|_| ()).map_err(Error::from)
        } else if descriptor == &definition["check"]["input"] {
            check_input(value)
        } else if descriptor == &definition["check"]["output"] {
            check_output(value)
        } else {
            Err(Error::Unsupported)
        }
    }

    // No Page operation interprets exceptional floating-point values.
    fn nonfinite(&self) -> bool {
        tracing::trace!("Declaring finite Page value semantics");
        false
    }
}

// The check input carries the visitor's request, the agreed price and the returned values; only their types are fixed here.
fn check_input(value: &Value) -> Result<(), Error> {
    let Value::Object(fields) = value else {
        return Err(Error::Invalid);
    };
    let get = |name: &str| fields.iter().find(|(n, _)| n == name).map(|(_, v)| v);
    let ok = matches!(get("requestedPath"), Some(Value::String(_)))
        && matches!(get("path"), Some(Value::String(_)))
        && matches!(get("mediaType"), Some(Value::String(_)))
        && matches!(get("contentSize"), Some(Value::Uint64(_)))
        && matches!(get("contentHash"), Some(Value::Bytes(_)))
        && matches!(get("content"), Some(Value::Bytes(_)))
        && matches!(get("agreedPrice"), Some(Value::Uint64(_)))
        && matches!(get("price"), Some(Value::Uint64(_)));
    if ok { Ok(()) } else { Err(Error::Invalid) }
}

// The visitor-computed conclusion: exactly `pass`, `fail` or `inconclusive`.
fn check_output(value: &Value) -> Result<(), Error> {
    let Value::Object(fields) = value else {
        return Err(Error::Invalid);
    };
    match fields.as_slice() {
        [(name, Value::String(outcome))]
            if name == "outcome"
                && matches!(outcome.as_str(), "pass" | "fail" | "inconclusive") =>
        {
            Ok(())
        }
        _ => Err(Error::Invalid),
    }
}

impl Requester for Client {
    // The exact definition installed with this code.
    fn definition(&self) -> &Definition {
        tracing::trace!("Borrowing the installed Page definition");
        &self.definition
    }

    // Economic understanding is independent of any reservation: the fixed price and the two limits must be valid.
    fn terms(&self, offering: &Offering) -> Result<(), Error> {
        tracing::trace!("Validating Page offering economics");
        Terms::parse(offering).map(|_| ()).map_err(Error::from)
    }

    // A result must belong to the request that was made and pass the local check at the accepted fixed price, before it can authorize settlement.
    fn validate_result(
        &self,
        invocation: &Invocation,
        input: &Value,
        output: &Value,
    ) -> Result<(), Error> {
        tracing::trace!("Checking a Page result against its request");
        let request = codec::parse_request(input).map_err(Error::from)?;
        let returned = codec::parse_output(output).map_err(Error::from)?;
        let terms = Terms::parse(&invocation.offering).map_err(Error::from)?;
        let consistent = returned.path == request.path
            && returned.content_size <= request.maximum_bytes
            && returned.content_size <= terms.maximum_resource_bytes
            && judge(input, output, Some(terms.price)) == Outcome::Pass;
        if consistent {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }

    // The local check: the visitor computes pass, fail or inconclusive itself from its request, the price it agreed to (the requester-owned expectation
    // `{agreedPrice}`) and the output it received. Nothing is sent to the publisher.
    fn verify(
        &self,
        input: &Value,
        _terms: &Blob,
        result: &Value,
        expectations: Option<&Value>,
    ) -> Result<Check, Error> {
        tracing::trace!("Running the local Page check");
        let agreed = expectations.and_then(|e| match e {
            Value::Object(fields) => fields.iter().find_map(|(n, v)| match (n.as_str(), v) {
                ("agreedPrice", Value::Uint64(price)) => Some(*price),
                _ => None,
            }),
            _ => None,
        });
        Ok(match judge(input, result, agreed) {
            Outcome::Pass => Check::Pass,
            Outcome::Fail => Check::Fail,
            Outcome::Inconclusive => Check::Inconclusive,
        })
    }
}

// Build the visitor's evidence from raw values, so an invalid label, path or hash is a failure and only a missing input is inconclusive.
fn judge(input: &Value, output: &Value, agreed_price: Option<u64>) -> Outcome {
    let field = |value: &Value, name: &str| match value {
        Value::Object(fields) => fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone()),
        _ => None,
    };
    let returned = (|| {
        Some(Returned {
            path: match field(output, "path")? {
                Value::String(text) => text,
                _ => return None,
            },
            media_type: match field(output, "mediaType")? {
                Value::String(text) => text,
                _ => return None,
            },
            content_size: match field(output, "contentSize")? {
                Value::Uint64(n) => n,
                _ => return None,
            },
            content_hash: match field(output, "contentHash")? {
                Value::Bytes(bytes) => bytes,
                _ => return None,
            },
            content: match field(output, "content")? {
                Value::Bytes(bytes) => bytes,
                _ => return None,
            },
            price: match field(output, "price")? {
                Value::Uint64(n) => n,
                _ => return None,
            },
        })
    })();
    let requested_path = match field(input, "path") {
        Some(Value::String(text)) => Some(text),
        _ => None,
    };
    check::evaluate(&Evidence {
        requested_path,
        agreed_price,
        returned,
    })
}
