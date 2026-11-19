//! Storage input projections and semantic checks retain exact known values without copying store payloads into quotes.
use super::Client;
use crate::modules::{
    identity::api::IdentityId,
    services::{
        Error,
        handler::Peer,
        validation::Semantics,
        value::{Budget, Value},
    },
    storage::operation::{self, Operation},
};
impl Semantics for Client {
    // Apply independent Storage operation restrictions only for this installed contract's own descriptors.
    fn validate(&self, descriptor: &serde_json::Value, value: &Value) -> Result<(), Error> {
        tracing::trace!("Applying installed Storage requester operation semantics");
        let definition = self.definition.document.value();
        let budget = self.policy.values;
        value.bounded(budget)?;
        if descriptor == &definition["input"] {
            operation::input(value, budget).map(|_| ())
        } else if descriptor == &definition["output"] {
            operation::output(value, kind(value)?, budget)
        } else if descriptor == &definition["check"]["input"] {
            operation::challenge(value, budget)
        } else if descriptor == &definition["check"]["output"] {
            check_output(value, budget)
        } else {
            Err(Error::Unsupported)
        }
    }
    // No Storage operation interprets exceptional floating-point values as protocol dimensions.
    fn nonfinite(&self) -> bool {
        tracing::trace!("Declaring finite Storage requester value semantics");
        false
    }
}
impl Client {
    // Keep large store data out of local quote metadata while checking its exact declared logical count.
    pub(super) fn metadata(&self, input: &Value, budget: Budget) -> Result<Value, Error> {
        tracing::trace!("Projecting exact original Storage requester operation metadata");
        let operation = operation::input(input, self.budget(budget))?;
        if operation != Operation::Store {
            return Ok(input.clone());
        }
        let Value::Bytes(data) = field(input, "data")? else {
            return Err(Error::Invalid);
        };
        let size = number(input, "sizeBytes")?;
        if u64::try_from(data.len()).map_err(|_| Error::Limit)? != size {
            return Err(Error::Invalid);
        }
        Ok(Value::Object(vec![
            ("operation".into(), Value::String("store".into())),
            ("sizeBytes".into(), Value::Uint64(size)),
            (
                "retentionSeconds".into(),
                Value::Uint64(number(input, "retentionSeconds")?),
            ),
        ]))
    }
}
// Derive application roles only from the trusted original peer/key association, independently of issuer and stream direction.
pub(super) fn roles(peer: Peer) -> Result<(IdentityId, IdentityId), Error> {
    tracing::trace!("Deriving Storage requester and provider from authenticated association");
    let requester = peer.key.requester;
    if peer.key.stream == 0 {
        return Err(Error::Denied);
    }
    let provider = if peer.local == requester {
        peer.remote
    } else if peer.remote == requester {
        peer.local
    } else {
        return Err(Error::Denied);
    };
    Ok((requester, provider))
}
// Read only an already bounded required logical object field without accepting duplicate names or payload authority.
pub(super) fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    tracing::trace!("Reading original Storage requester logical field");
    let Value::Object(fields) = value else {
        return Err(Error::Invalid);
    };
    fields
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .ok_or(Error::Invalid)
}
// Preserve all uint64 bits for counts, prices and retention values.
pub(super) fn number(value: &Value, name: &str) -> Result<u64, Error> {
    tracing::trace!("Reading exact original Storage requester integer");
    match field(value, name)? {
        Value::Uint64(number) => Ok(*number),
        _ => Err(Error::Invalid),
    }
}
// Discriminate output without pretending it is a complete input, whose required field set is different.
pub(super) fn kind(value: &Value) -> Result<Operation, Error> {
    tracing::trace!("Reading understood Storage requester operation discriminator");
    match field(value, "operation")? {
        Value::String(name) => match name.as_str() {
            "store" => Ok(Operation::Store),
            "retrieve" => Ok(Operation::Retrieve),
            "renew" => Ok(Operation::Renew),
            "remove" => Ok(Operation::Remove),
            _ => Err(Error::Invalid),
        },
        _ => Err(Error::Invalid),
    }
}
// Logical check conclusions belong to the requester; provider evidence uses separate Rules and cannot supply outcome.
fn check_output(value: &Value, budget: Budget) -> Result<(), Error> {
    tracing::trace!("Validating requester-owned Storage logical check conclusion");
    value.bounded(budget)?;
    if !matches!(field(value,"outcome")?,Value::String(outcome) if matches!(outcome.as_str(),"pass"|"fail"|"inconclusive"))
        || !matches!(field(value,"leaseId")?,Value::Bytes(id) if id.len()==32)
    {
        return Err(Error::Invalid);
    }
    number(value, "offset")?;
    let Value::Object(fields) = value else {
        return Err(Error::Invalid);
    };
    if fields.iter().any(|(name, value)| {
        name == "value" && !matches!(value,Value::Bytes(byte) if byte.len()==1)
    }) {
        return Err(Error::Invalid);
    }
    Ok(())
}
