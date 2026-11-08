//! Test-owned echo family implements public ports; no production authentication or provider registration.
use super::super::{
    Error, contract::OperationKey, handler::*, validation::Semantics, value::Value,
};
use crate::modules::board::{
    config::Limits,
    definition::Definition,
    offering::{Board, Offering},
};
use std::{collections::BTreeMap, sync::Mutex};

pub(in crate::modules::services) struct Echo {
    definition: Definition,
    configured: Option<(Value, u64)>,
    state: Mutex<BTreeMap<OperationKey, (Completion, Status)>>,
    pub(in crate::modules::services) executions: std::sync::atomic::AtomicUsize,
}
// Construct complete literal custom contracts through Board's public parsers.
pub(in crate::modules::services) fn offering(wanted: bool) -> Offering {
    let issuer =
        crate::modules::identity::text::format(crate::modules::identity::api::IdentityId([2; 32]));
    let o = serde_json::json!({"id":"echo","service":"org.example.echo","serviceVersion":1,"description":"Return the original text.","price":7,"creditIssuer":issuer});
    let board = if wanted {
        serde_json::json!({"version":1,"providedServices":[],"wantedServices":[o]})
    } else {
        serde_json::json!({"version":1,"providedServices":[o],"wantedServices":[]})
    };
    Board::parse(board.to_string().as_bytes(), Limits::default())
        .unwrap()
        .offerings
        .remove("echo")
        .unwrap()
}
impl Echo {
    // Script a literal invalid provider completion independently of production validation.
    pub(in crate::modules::services) fn with_output(output: Value, price: u64) -> Self {
        let mut family = Self::new();
        family.configured = Some((output, price));
        family
    }
    // A synthetic implementation must be installed explicitly; descriptive text cannot construct it.
    pub(in crate::modules::services) fn new() -> Self {
        let raw=br#"{"id":"echo","service":"org.example.echo","serviceVersion":1,"description":"Return the original text.","input":{"type":"string"},"output":{"type":"string"},"check":{"description":"Compare returned text to original.","input":{"type":"string"},"output":{"type":"bool"}}}"#;
        Self {
            configured: None,
            definition: Definition::parse(raw, &offering(false), Limits::default()).unwrap(),
            state: Mutex::new(BTreeMap::new()),
            executions: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}
// Fixture tokens bind owner, original attempt and staged effect independently of response delivery.
fn token(key: OperationKey, effect: &Blob) -> Blob {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"Services echo fixture owner B");
    hash.update(key.requester.0);
    hash.update(key.connection);
    hash.update(key.stream.to_be_bytes());
    hash.update(effect.bytes());
    Blob::new(hash.finalize().to_vec(), 32).unwrap()
}
impl Semantics for Echo {
    // Exact echo data is text; unfamiliar required schema semantics cannot be accepted.
    fn validate(&self, s: &serde_json::Value, v: &Value) -> Result<(), Error> {
        if s.get("requiredExtension").is_some() {
            return Err(Error::Unsupported);
        }
        if !matches!(v, Value::String(_)) {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    // Echo has no nonfinite arithmetic semantics.
    fn nonfinite(&self) -> bool {
        false
    }
}
impl Requester for Echo {
    // The fixture owns an immutable independently literal contract definition.
    fn definition(&self) -> &Definition {
        &self.definition
    }
    // Only fixed price7 is compatible with this deliberately narrow fixture.
    fn terms(&self, o: &Offering) -> Result<(), Error> {
        if crate::modules::board::offering::integer(&o.source["price"]) == Some(7) {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
    }
    // Missing expectations cannot establish success; complete mismatching evidence establishes failure.
    fn verify(
        &self,
        input: &Value,
        _: &Blob,
        result: &Value,
        expected: Option<&Value>,
    ) -> Result<Check, Error> {
        Ok(match expected {
            None => Check::Inconclusive,
            Some(e) if e == input && result == e => Check::Pass,
            Some(_) => Check::Fail,
        })
    }
}
impl Provider for Echo {
    // Assessment returns literal factors and an explicit text limit without effects.
    fn assess(&self, _: Peer, _: &Offering, metadata: &Value) -> Result<Assessment, Error> {
        self.validate(&serde_json::json!({"type":"string"}), metadata)?;
        Ok(Assessment {
            metadata: metadata.clone(),
            factors: Value::Object(vec![]),
            input_bytes: 128,
            pin: None,
        })
    }
    // Execution stages exactly one result; duplicate dispatch is detected rather than hidden.
    fn execute(&self, i: &Invocation, input: &Value) -> Result<Completion, Error> {
        let mut state = self.state.lock().unwrap();
        if state.contains_key(&i.key) {
            return Err(Error::Conflict);
        }
        let Value::String(s) = input else {
            return Err(Error::Invalid);
        };
        let c = Completion {
            output: self
                .configured
                .as_ref()
                .map_or_else(|| input.clone(), |(v, _)| v.clone()),
            result: Blob::new(s.as_bytes().to_vec(), 128)?,
            price: self.configured.as_ref().map_or(7, |(_, p)| *p),
            effect: Blob::new(vec![7], 1)?,
        };
        self.executions
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        state.insert(i.key, (c.clone(), Status::Absent));
        Ok(c)
    }
    // Recover an already staged result without invoking execution again.
    fn completion(&self, k: OperationKey) -> Result<Completion, Error> {
        self.state
            .lock()
            .unwrap()
            .get(&k)
            .map(|(c, _)| c.clone())
            .ok_or(Error::Unknown)
    }
    // Tokens are exact fixture effect references; production handlers must supply durable scoped tokens.
    fn prepare(&self, k: OperationKey, e: &Blob) -> Result<Blob, Error> {
        let mut s = self.state.lock().unwrap();
        let (c, status) = s.get_mut(&k).ok_or(Error::Unknown)?;
        if &c.effect != e {
            return Err(Error::Conflict);
        }
        if matches!(status, Status::Aborted) {
            return Err(Error::Conflict);
        }
        if !matches!(status, Status::Committed) {
            *status = Status::Prepared(token(k, e));
        }
        Ok(token(k, e))
    }
    // Duplicate commit is idempotent; missing preparation never manufactures successful work.
    fn commit(&self, k: OperationKey, t: &Blob) -> Result<(), Error> {
        let mut s = self.state.lock().unwrap();
        let (c, status) = s.get_mut(&k).ok_or(Error::Unknown)?;
        if &token(k, &c.effect) != t || !matches!(status, Status::Prepared(_) | Status::Committed) {
            return Err(Error::Conflict);
        }
        *status = Status::Committed;
        Ok(())
    }
    // Only an explicit coordinator abort may discard staged work.
    fn abort(&self, k: OperationKey, t: Option<&Blob>) -> Result<(), Error> {
        let mut s = self.state.lock().unwrap();
        if let Some((c, status)) = s.get_mut(&k) {
            if matches!(status, Status::Committed) {
                return Err(Error::Conflict);
            }
            if matches!(status, Status::Prepared(_)) && t != Some(&token(k, &c.effect)) {
                return Err(Error::Denied);
            }
            *status = Status::Aborted;
        }
        Ok(())
    }
    // Return participant state without converting absence or reply loss into rollback.
    fn status(&self, k: OperationKey) -> Result<Status, Error> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .get(&k)
            .map(|(_, s)| s.clone())
            .unwrap_or(Status::Absent))
    }
    // Echo's check is requester-local, so no remote evidence operation exists.
    fn evidence(&self, _: &Invocation, _: &Value) -> Result<Value, Error> {
        Err(Error::Unsupported)
    }
}

impl Echo {
    // Snapshot only complete durable family records and the independent execution counter.
    pub(in crate::modules::services) fn snapshot(&self) -> Result<Vec<u8>, Error> {
        let state = self.state.lock().unwrap();
        let rows = state
            .iter()
            .map(|(k, (c, s))| {
                Value::Array(vec![
                    Value::Bytes(k.requester.0.to_vec()),
                    Value::Bytes(k.connection.to_vec()),
                    Value::Uint64(u64::from(k.stream)),
                    c.output.clone(),
                    Value::Bytes(c.result.bytes().to_vec()),
                    Value::Uint64(c.price),
                    Value::Bytes(c.effect.bytes().to_vec()),
                    Value::Uint64(match s {
                        Status::Absent => 0,
                        Status::Prepared(_) => 1,
                        Status::Committed => 2,
                        Status::Aborted => 3,
                    }),
                ])
            })
            .collect();
        super::super::codec::encode(
            &Value::Array(vec![
                Value::Uint64(self.executions.load(std::sync::atomic::Ordering::SeqCst) as u64),
                Value::Array(rows),
            ]),
            super::super::value::Budget {
                depth: 16,
                nodes: 65536,
                bytes: 1_000_000,
            },
            1_000_000,
        )
    }
    // Reconstruct a new independent participant solely from validated snapshot bytes, never shared state/locks.
    pub(in crate::modules::services) fn restore(bytes: &[u8]) -> Result<Self, Error> {
        let root = super::super::codec::decode(
            bytes,
            super::super::value::Budget {
                depth: 16,
                nodes: 65536,
                bytes: 1_000_000,
            },
            1_000_000,
        )?;
        let Value::Array(root) = root else {
            return Err(Error::Invalid);
        };
        let [Value::Uint64(count), Value::Array(rows)] = root.as_slice() else {
            return Err(Error::Invalid);
        };
        let mut state = BTreeMap::new();
        for row in rows {
            let Value::Array(row) = row else {
                return Err(Error::Invalid);
            };
            let [
                Value::Bytes(requester),
                Value::Bytes(connection),
                Value::Uint64(stream),
                output,
                Value::Bytes(result),
                Value::Uint64(price),
                Value::Bytes(effect),
                Value::Uint64(status),
            ] = row.as_slice()
            else {
                return Err(Error::Invalid);
            };
            let key = OperationKey {
                requester: crate::modules::identity::api::IdentityId(
                    requester
                        .as_slice()
                        .try_into()
                        .map_err(|_| Error::Invalid)?,
                ),
                connection: connection
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Invalid)?,
                stream: u32::try_from(*stream).map_err(|_| Error::Invalid)?,
            };
            let c = Completion {
                output: output.clone(),
                result: Blob::new(result.clone(), 1_000_000)?,
                price: *price,
                effect: Blob::new(effect.clone(), 1_000_000)?,
            };
            let status = match status {
                0 => Status::Absent,
                1 => Status::Prepared(token(key, &c.effect)),
                2 => Status::Committed,
                3 => Status::Aborted,
                _ => return Err(Error::Invalid),
            };
            if state.insert(key, (c, status)).is_some() {
                return Err(Error::Invalid);
            }
        }
        if usize::try_from(*count).ok() != Some(state.len()) {
            return Err(Error::Invalid);
        }
        let family = Self::new();
        *family.state.lock().unwrap() = state;
        family
            .executions
            .store(*count as usize, std::sync::atomic::Ordering::SeqCst);
        Ok(family)
    }
}
