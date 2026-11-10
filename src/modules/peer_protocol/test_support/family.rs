//! Explicit test-owned fixed-price text/byte echo ports; no production provider or authentication is installed.
use crate::modules::{
    board::{
        self,
        definition::Definition,
        offering::{Board, Offering},
    },
    identity::{api::IdentityId, text},
    services::{
        Error,
        contract::OperationKey,
        handler::*,
        input::Input,
        validation::Semantics,
        value::{Budget, Value},
    },
};
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
pub(in crate::modules::peer_protocol) struct Echo {
    definition: Definition,
    bytes: bool,
    state: Mutex<BTreeMap<OperationKey, (Completion, Status)>>,
    pub(in crate::modules::peer_protocol) assessments: AtomicUsize,
    pub(in crate::modules::peer_protocol) executions: AtomicUsize,
}
impl Echo {
    // Construct a literal custom contract independently of the Peer Protocol codec.
    pub(in crate::modules::peer_protocol) fn new(bytes: bool) -> Self {
        let service = if bytes {
            "org.example.peer-bytes"
        } else {
            "org.example.peer-test"
        };
        let description = if bytes {
            "Return immutable bytes."
        } else {
            "Metadata test."
        };
        let kind = if bytes { "bytes" } else { "string" };
        let raw = serde_json::json!({"id":"fixture","service":service,"serviceVersion":1,"description":description,"input":{"type":kind},"output":{"type":kind},"check":{"description":"Compare retained output locally.","input":{"type":kind},"output":{"type":"bool"}}});
        let o = Self::make_offering(bytes, IdentityId([2; 32]));
        Self {
            definition: Definition::parse(raw.to_string().as_bytes(), &o, Default::default())
                .unwrap(),
            bytes,
            state: Mutex::new(BTreeMap::new()),
            assessments: AtomicUsize::new(0),
            executions: AtomicUsize::new(0),
        }
    }
    // Offering economics are literal fixed seven and an explicit issuer; definition schemas contain none of them.
    fn make_offering(bytes: bool, issuer: IdentityId) -> Offering {
        let service = if bytes {
            "org.example.peer-bytes"
        } else {
            "org.example.peer-test"
        };
        let description = if bytes {
            "Return immutable bytes."
        } else {
            "Metadata test."
        };
        let raw = serde_json::json!({"version":1,"providedServices":[{"id":"fixture","service":service,"serviceVersion":1,"description":description,"price":7,"creditIssuer":text::format(issuer)}],"wantedServices":[]});
        Board::parse(raw.to_string().as_bytes(), Default::default())
            .unwrap()
            .offerings
            .remove("fixture")
            .unwrap()
    }
    // Each world chooses its actual configured issuer without redefining this custom logical contract.
    pub(in crate::modules::peer_protocol) fn offering(&self, issuer: IdentityId) -> Offering {
        Self::make_offering(self.bytes, issuer)
    }
}
// Exact effect tokens bind fixture owner/key and effect; no raw process pointers or filesystem paths escape.
fn token(key: OperationKey, effect: &Blob) -> Blob {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"Peer Protocol test echo owner");
    h.update(key.requester.0);
    h.update(key.connection);
    h.update(key.stream.to_be_bytes());
    h.update(effect.bytes());
    Blob::new(h.finalize().to_vec(), 32).unwrap()
}
impl Semantics for Echo {
    // Text and byte contracts remain disjoint rather than accepting merely a matching service name.
    fn validate(&self, _: &serde_json::Value, value: &Value) -> Result<(), Error> {
        if matches!(
            (self.bytes, value),
            (true, Value::Bytes(_)) | (false, Value::String(_))
        ) {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
    // This fixture has no nonfinite numeric semantics.
    fn nonfinite(&self) -> bool {
        false
    }
}
impl Requester for Echo {
    // The literal contract is immutable independently of offering economics.
    fn definition(&self) -> &Definition {
        &self.definition
    }
    // Fixed seven is the only accepted model for this installed fixture.
    fn terms(&self, o: &Offering) -> Result<(), Error> {
        if board::offering::integer(&o.source["price"]) == Some(7) {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
    }
    // Byte and UTF-8 text metadata project exact logical byte length independently.
    fn assessment_metadata(&self, input: &Value, budget: Budget) -> Result<Value, Error> {
        input.bounded(budget)?;
        self.validate(&serde_json::Value::Null, input)?;
        Ok(Value::Uint64(input.byte_len(budget)? as u64))
    }
    // Remote factors are empty and cannot fabricate input or inflate its independently known size.
    fn validate_assessment(
        &self,
        input: &Value,
        metadata: &Value,
        factors: &Value,
        budget: Budget,
    ) -> Result<(), Error> {
        if &self.assessment_metadata(input, budget)? != metadata
            || factors != &Value::Object(vec![])
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    // Complete input must match its accepted size/text metadata, with no placeholder content during assessment.
    fn validate_input(&self, metadata: &Value, input: &Value, budget: Budget) -> Result<(), Error> {
        if &self.assessment_metadata(input, budget)? == metadata {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
    // This custom echo promises original content; requester-side received output is independently compared before settlement.
    fn validate_result(&self, _: &Invocation, input: &Value, output: &Value) -> Result<(), Error> {
        if input == output {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
    // Property results are computed locally and cannot alter credits or trust.
    fn verify(
        &self,
        input: &Value,
        _: &Blob,
        result: &Value,
        expected: Option<&Value>,
    ) -> Result<Check, Error> {
        Ok(match expected {
            None => Check::Inconclusive,
            Some(e) if e == input && e == result => Check::Pass,
            Some(_) => Check::Fail,
        })
    }
}
impl Provider for Echo {
    // Complete metadata goes through the same immutable length/text projection as the requester.
    fn assess(&self, _: Peer, _: &Offering, metadata: &Value) -> Result<Assessment, Error> {
        self.assessments.fetch_add(1, Ordering::SeqCst);
        let budget = Budget {
            depth: 31,
            nodes: 65536,
            bytes: 1_048_576,
        };
        let projected = self.assessment_metadata(metadata, budget)?;
        Ok(Assessment {
            metadata: projected,
            factors: Value::Object(vec![]),
            input_bytes: metadata.byte_len(budget)? as u64,
            pin: None,
        })
    }
    // Declared byte length can be assessed without owning or inventing its payload.
    fn assess_metadata(
        &self,
        peer: Peer,
        o: &Offering,
        metadata: &Input,
        budget: Budget,
    ) -> Result<Assessment, Error> {
        metadata.bounded(budget)?;
        if let Input::Body { length, text } = metadata {
            if *text == self.bytes || *length > 1_048_576 {
                return Err(Error::Limit);
            }
            self.assessments.fetch_add(1, Ordering::SeqCst);
            Ok(Assessment {
                metadata: Value::Uint64(*length),
                factors: Value::Object(vec![]),
                input_bytes: *length,
                pin: None,
            })
        } else {
            self.assess(peer, o, &metadata.complete(budget)?)
        }
    }
    // Stage once after complete validation; duplicate dispatch is an error rather than a hidden work retry.
    fn execute(&self, i: &Invocation, input: &Value) -> Result<Completion, Error> {
        let mut state = self.state.lock().unwrap();
        if state.contains_key(&i.key) {
            return Err(Error::Conflict);
        }
        self.validate(&serde_json::Value::Null, input)?;
        let bytes = match input {
            Value::Bytes(b) => b.clone(),
            Value::String(s) => s.as_bytes().to_vec(),
            _ => return Err(Error::Invalid),
        };
        let c = Completion {
            output: input.clone(),
            result: Blob::new(bytes, 1_048_576)?,
            price: 7,
            effect: Blob::new(vec![7], 1)?,
        };
        self.executions.fetch_add(1, Ordering::SeqCst);
        state.insert(i.key, (c.clone(), Status::Absent));
        Ok(c)
    }
    // Recovery reads staged data; it never executes the input again.
    fn completion(&self, key: OperationKey) -> Result<Completion, Error> {
        self.state
            .lock()
            .unwrap()
            .get(&key)
            .map(|(c, _)| c.clone())
            .ok_or(Error::Unknown)
    }
    // Preparation binds exactly the staged effect and retains committed/aborted decisions.
    fn prepare(&self, key: OperationKey, effect: &Blob) -> Result<Blob, Error> {
        let mut s = self.state.lock().unwrap();
        let (c, status) = s.get_mut(&key).ok_or(Error::Unknown)?;
        if &c.effect != effect || matches!(status, Status::Aborted) {
            return Err(Error::Conflict);
        }
        let t = token(key, effect);
        if !matches!(status, Status::Committed) {
            *status = Status::Prepared(t.clone());
        }
        Ok(t)
    }
    // Matching duplicate decision delivery is idempotent and cannot manufacture missing preparation.
    fn commit(&self, key: OperationKey, t: &Blob) -> Result<(), Error> {
        let mut s = self.state.lock().unwrap();
        let (c, status) = s.get_mut(&key).ok_or(Error::Unknown)?;
        if &token(key, &c.effect) != t || !matches!(status, Status::Prepared(_) | Status::Committed)
        {
            return Err(Error::Conflict);
        }
        *status = Status::Committed;
        Ok(())
    }
    // Explicit abort preserves any committed effect and checks the exact prepared token.
    fn abort(&self, key: OperationKey, t: Option<&Blob>) -> Result<(), Error> {
        if let Some((c, status)) = self.state.lock().unwrap().get_mut(&key) {
            if matches!(status, Status::Committed) {
                return Err(Error::Conflict);
            }
            if matches!(status, Status::Prepared(_)) && t != Some(&token(key, &c.effect)) {
                return Err(Error::Denied);
            }
            *status = Status::Aborted;
        }
        Ok(())
    }
    // Absence is an observation only, never proof that an unknown external action rolled back.
    fn status(&self, key: OperationKey) -> Result<Status, Error> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .get(&key)
            .map(|(_, s)| s.clone())
            .unwrap_or(Status::Absent))
    }
    // The fixture verifier is local-only, so remote evidence is unsupported.
    fn evidence(
        &self,
        _: &crate::modules::services::checks::Invocation,
        _: &Value,
    ) -> Result<Value, Error> {
        Err(Error::Unsupported)
    }
}
