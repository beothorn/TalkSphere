//! Explicit volatile custom text handler shared by the local Services and authenticated Peer Protocol examples.
use std::{collections::BTreeMap, sync::Mutex};
use talksphere::modules::{
    board::{
        definition::Definition,
        offering::{Board, Offering},
    },
    services::{Error, contract::OperationKey, handler::*, validation::Semantics, value::Value},
};

pub(super) struct Echo {
    definition: Definition,
    state: Mutex<BTreeMap<OperationKey, (Completion, Status)>>,
}
impl Echo {
    // Parse independent complete custom contracts; no standard family or runtime is installed.
    pub(super) fn new(issuer: String) -> Result<(Self, Offering, Vec<u8>), String> {
        tracing::trace!("Preparing explicitly installed local example contract");
        let offering = serde_json::json!({"id":"echo","service":"org.example.echo","serviceVersion":1,"description":"Return the original text.","price":7,"creditIssuer":issuer});
        let bytes =
            serde_json::json!({"version":1,"providedServices":[offering],"wantedServices":[]})
                .to_string()
                .into_bytes();
        let mut board = Board::parse(&bytes, Default::default()).map_err(super::failure)?;
        let offering = board
            .offerings
            .remove("echo")
            .ok_or("Missing example offering")?;
        let raw = br#"{"id":"echo","service":"org.example.echo","serviceVersion":1,"description":"Return the original text.","input":{"type":"string"},"output":{"type":"string"},"check":{"description":"Compare returned text locally.","input":{"type":"string"},"output":{"type":"bool"}}}"#;
        let definition =
            Definition::parse(raw, &offering, Default::default()).map_err(super::failure)?;
        Ok((
            Self {
                definition,
                state: Mutex::new(BTreeMap::new()),
            },
            offering,
            bytes,
        ))
    }
}
impl Semantics for Echo {
    // Text has no permission for nonfinite numeric values.
    fn nonfinite(&self) -> bool {
        tracing::trace!("Selecting example numeric policy");
        false
    }

    // This custom contract accepts text and has no extension-dependent semantics.
    fn validate(&self, _: &serde_json::Value, value: &Value) -> Result<(), Error> {
        tracing::trace!("Validating example text value");
        if matches!(value, Value::String(_)) {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
}
impl Requester for Echo {
    // Return the immutable independently installed contract.
    fn definition(&self) -> &Definition {
        tracing::trace!("Reading example contract");
        &self.definition
    }
    // Only the exact fixed price seven belongs to this example family.
    fn terms(&self, offering: &Offering) -> Result<(), Error> {
        tracing::trace!("Checking example fixed price");
        if talksphere::modules::board::offering::integer(&offering.source["price"]) == Some(7) {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
    }
    // Compare only requester-private expectations; missing expectations are inconclusive.
    fn verify(
        &self,
        input: &Value,
        _: &Blob,
        result: &Value,
        expected: Option<&Value>,
    ) -> Result<Check, Error> {
        tracing::trace!("Comparing example result with local expectations");
        Ok(match expected {
            None => Check::Inconclusive,
            Some(e) if e == input && e == result => Check::Pass,
            Some(_) => Check::Fail,
        })
    }
}
// A volatile participant token binds the original owner/key and effect; it is never network authentication.
fn token(key: OperationKey, effect: &Blob) -> Result<Blob, Error> {
    use sha2::{Digest, Sha256};
    tracing::trace!("Binding local example effect token");
    let mut hash = Sha256::new();
    hash.update(b"TalkSphere volatile Services example");
    hash.update(key.requester.0);
    hash.update(key.connection);
    hash.update(key.stream.to_be_bytes());
    hash.update(effect.bytes());
    Blob::new(hash.finalize().to_vec(), 32)
}
impl Provider for Echo {
    // Metadata validation happens before any execution or reservation.
    fn assess(&self, _: Peer, _: &Offering, metadata: &Value) -> Result<Assessment, Error> {
        tracing::trace!("Assessing bounded local example metadata");
        self.validate(&serde_json::Value::Null, metadata)?;
        Ok(Assessment {
            metadata: metadata.clone(),
            factors: Value::Object(vec![]),
            input_bytes: 128,
            pin: None,
        })
    }
    // Retain one complete text result; this example deliberately uses volatile in-memory participant state.
    fn execute(&self, invocation: &Invocation, input: &Value) -> Result<Completion, Error> {
        tracing::trace!("Executing explicit local text demonstration");
        let Value::String(text) = input else {
            return Err(Error::Invalid);
        };
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        if state.contains_key(&invocation.key) {
            return Err(Error::Conflict);
        }
        let c = Completion {
            output: input.clone(),
            result: Blob::new(text.as_bytes().to_vec(), 128)?,
            price: 7,
            effect: Blob::new(vec![7], 1)?,
        };
        state.insert(invocation.key, (c.clone(), Status::Absent));
        Ok(c)
    }
    // Read retained completion without running text work again.
    fn completion(&self, key: OperationKey) -> Result<Completion, Error> {
        tracing::trace!("Reading retained example completion");
        self.state
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(&key)
            .map(|(c, _)| c.clone())
            .ok_or(Error::Unknown)
    }
    // Prepare only the exact staged effect, preserving already committed state.
    fn prepare(&self, key: OperationKey, effect: &Blob) -> Result<Blob, Error> {
        tracing::trace!("Preparing local example effect");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        let (c, status) = state.get_mut(&key).ok_or(Error::Unknown)?;
        if &c.effect != effect || matches!(status, Status::Aborted) {
            return Err(Error::Conflict);
        }
        let token = token(key, effect)?;
        if !matches!(status, Status::Committed) {
            *status = Status::Prepared(token.clone());
        }
        Ok(token)
    }
    // Deliver a matching token once; duplicate commit remains idempotent.
    fn commit(&self, key: OperationKey, t: &Blob) -> Result<(), Error> {
        tracing::trace!("Committing prepared local example effect");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        let (c, status) = state.get_mut(&key).ok_or(Error::Unknown)?;
        if &token(key, &c.effect)? != t
            || !matches!(status, Status::Prepared(_) | Status::Committed)
        {
            return Err(Error::Conflict);
        }
        *status = Status::Committed;
        Ok(())
    }
    // Explicit abort never undoes commitment and requires the original prepared token.
    fn abort(&self, key: OperationKey, t: Option<&Blob>) -> Result<(), Error> {
        tracing::trace!("Aborting uncommitted local example effect");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        if let Some((c, status)) = state.get_mut(&key) {
            if matches!(status, Status::Committed) {
                return Err(Error::Conflict);
            }
            if matches!(status, Status::Prepared(_)) && t != Some(&token(key, &c.effect)?) {
                return Err(Error::Denied);
            }
            *status = Status::Aborted;
        }
        Ok(())
    }
    // Query exact volatile participant state without inferring remote receipt.
    fn status(&self, key: OperationKey) -> Result<Status, Error> {
        tracing::trace!("Observing local example participant");
        Ok(self
            .state
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(&key)
            .map(|(_, s)| s.clone())
            .unwrap_or(Status::Absent))
    }
    // The custom example defines only a local verifier, never a remote evidence service.
    fn evidence(
        &self,
        _: &talksphere::modules::services::checks::Invocation,
        _: &Value,
    ) -> Result<Value, Error> {
        tracing::trace!("Refusing absent example evidence provider");
        Err(Error::Unsupported)
    }
}
