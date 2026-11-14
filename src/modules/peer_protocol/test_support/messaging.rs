//! Narrow pinned-contract replay fixture, not a Messaging resource/signature implementation or production registration.
use super::{family::Echo, world::Family};
use crate::modules::{
    board::{self, definition::Definition, offering::Offering},
    identity::{api::IdentityId, text},
    services::{
        self, Error,
        contract::OperationKey,
        handler::*,
        replay::{Id, Purpose},
        validation::Semantics,
        value::{Budget, Value},
    },
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
const DEFINITION: &str = include_str!("messaging-definition.json");
struct Catalogue;
impl services::registry::Standards for Catalogue {
    // This fixture compares the complete pinned logical Messaging definition; it installs no real Messaging backend.
    fn validate(&self, definition: &Definition) -> Result<(), Error> {
        let pinned: serde_json::Value = serde_json::from_str(DEFINITION).unwrap();
        if definition.document.value() == &pinned {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
    }
}
pub(in crate::modules::peer_protocol) struct Messaging {
    definition: Definition,
    effects: Echo,
    results: Mutex<BTreeMap<OperationKey, Completion>>,
    pub prices: AtomicUsize,
    pub cache_reads: AtomicUsize,
    pub allow: AtomicBool,
}
// Fixture immutable payload bytes and their hash exercise detached cached output without claiming signature verification.
fn payload() -> Vec<u8> {
    vec![9; 5000]
}
// The complete pinned catalogue is independent of the Peer Protocol value codecs.
fn offering(issuer: IdentityId) -> Offering {
    let raw = json!({"version":1,"providedServices":[{"id":"messaging-standard","service":"cr2se.messaging","serviceVersion":1,"description":"Retain signed messages until recipient recovery or expiration.","creditIssuer":text::format(issuer),"pricing":{"model":"cr2se.messaging.v1","minimumPrice":7,"payloadByteUsageFactor":{"numerator":0,"denominator":1},"payloadByteStorageDurationFactor":{"numerator":0,"denominator":1},"recoveryByteUsageFactor":{"numerator":0,"denominator":1},"renewalByteStorageDurationFactor":{"numerator":0,"denominator":1},"discoverPrice":1,"statusPrice":1,"removePrice":7},"checkPrice":3,"info":{"maximumPayloadBytes":10000,"minimumRetentionSeconds":1,"maximumRetentionSeconds":1000,"maximumTotalRetentionSeconds":1000,"maximumDiscoverResults":1,"checkResponseSeconds":1},"preconditions":["cr2se.identity"]}],"wantedServices":[]});
    board::offering::Board::parse(raw.to_string().as_bytes(), Default::default())
        .unwrap()
        .offerings
        .remove("messaging-standard")
        .unwrap()
}
impl Messaging {
    // No catalogue, provider, state or replay permission is available unless a test explicitly constructs this owner.
    pub fn new() -> Self {
        Self {
            definition: Definition::parse(
                DEFINITION.as_bytes(),
                &offering(IdentityId([2; 32])),
                Default::default(),
            )
            .unwrap(),
            effects: Echo::new(false),
            results: Mutex::new(BTreeMap::new()),
            prices: AtomicUsize::new(0),
            cache_reads: AtomicUsize::new(0),
            allow: AtomicBool::new(true),
        }
    }
    // The fixture's participant counter detects accidental new execution on cached retries.
    pub fn executions(&self) -> usize {
        self.effects.executions.load(Ordering::SeqCst)
    }
    // Recovery/removal inputs include every normative field; fixed signature bytes are explicitly unverified fixture data.
    pub fn input(remove: bool) -> Value {
        use sha2::{Digest, Sha256};
        let mut fields: Vec<(String, Value)> = vec![
            (
                "operation".into(),
                Value::String(if remove { "remove" } else { "recover" }.into()),
            ),
            ("placementId".into(), Value::Bytes(vec![1; 32])),
        ];
        if remove {
            fields.extend([
                ("removalId".into(), Value::Bytes(vec![2; 32])),
                ("expectedExpiresAt".into(), Value::Uint64(100)),
                ("minimumReleaseCredits".into(), Value::Uint64(0)),
            ]);
        } else {
            fields.extend([
                ("messageId".into(), Value::Bytes(vec![3; 32])),
                (
                    "payloadHash".into(),
                    Value::Bytes(Sha256::digest(payload()).to_vec()),
                ),
                ("recoveryId".into(), Value::Bytes(vec![2; 32])),
                ("recipientPublicKey".into(), Value::Bytes(vec![4; 32])),
                ("recipientSignature".into(), Value::Bytes(vec![5; 64])),
            ]);
        }
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        Value::Object(fields)
    }
}
// Read one complete required logical field without coercing types or inventing absent request data.
fn field<'a>(input: &'a Value, name: &str) -> Result<&'a Value, Error> {
    let Value::Object(fields) = input else {
        return Err(Error::Invalid);
    };
    fields
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .ok_or(Error::Invalid)
}
impl Family for Messaging {
    // Tests choose the actual issuer independently on each local owner's real Board.
    fn offering(&self, issuer: IdentityId) -> Offering {
        offering(issuer)
    }
    // Standard-name installation is explicitly bounded to the pinned full catalogue in this fixture only.
    fn standards() -> Arc<dyn services::registry::Standards> {
        Arc::new(Catalogue)
    }
}
impl Semantics for Messaging {
    // Logical fixture operations preserve the full normative field shapes; cryptographic validation belongs to the future Messaging implementation.
    fn validate(&self, _: &serde_json::Value, value: &Value) -> Result<(), Error> {
        if matches!(field(value,"operation")?,Value::String(s) if matches!(s.as_str(),"recover"|"remove"))
        {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
    }
    // Messaging fixture values contain no nonfinite numbers.
    fn nonfinite(&self) -> bool {
        false
    }
}
impl Requester for Messaging {
    // Definitions stay pinned while publication/price callbacks may be drained or disabled in replay tests.
    fn definition(&self) -> &Definition {
        &self.definition
    }
    // Ordinary negotiation validates the fixture model and deliberately counts callbacks to prove retries bypass them.
    fn terms(&self, o: &Offering) -> Result<(), Error> {
        self.prices.fetch_add(1, Ordering::SeqCst);
        if o.source["pricing"]["model"] == "cr2se.messaging.v1" {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
    }
    // The fixture's original independently agreed charge is always seven; current callbacks are never used for replay.
    fn pricing(&self, o: &Offering, _: &Value) -> Result<services::pricing::Price, Error> {
        self.terms(o)?;
        Ok(services::pricing::Price::Fixed(7))
    }
    // Explicit family scope is the placement and recovery/removal ID, not an arbitrary generic invocation hash.
    fn cached_retry(&self, input: &Value) -> Result<Option<Id>, Error> {
        let purpose = match field(input, "operation")? {
            Value::String(s) if s == "recover" => Purpose::Recovery,
            Value::String(s) if s == "remove" => Purpose::Removal,
            _ => return Ok(None),
        };
        let name = if purpose == Purpose::Recovery {
            "recoveryId"
        } else {
            "removalId"
        };
        let Value::Bytes(placement) = field(input, "placementId")? else {
            return Err(Error::Invalid);
        };
        let Value::Bytes(id) = field(input, name)? else {
            return Err(Error::Invalid);
        };
        Ok(Some(Id {
            purpose,
            placement: placement
                .as_slice()
                .try_into()
                .map_err(|_| Error::Invalid)?,
            bytes: id.as_slice().try_into().map_err(|_| Error::Invalid)?,
        }))
    }
    // Provider factors are known fixture payload size or zero early release; they cannot redefine authorized input.
    fn validate_assessment(
        &self,
        input: &Value,
        metadata: &Value,
        factors: &Value,
        budget: Budget,
    ) -> Result<(), Error> {
        input.bounded(budget)?;
        if input != metadata {
            return Err(Error::Invalid);
        }
        let expected = if field(input, "operation")? == &Value::String("recover".into()) {
            Value::Object(vec![("payloadSize".into(), Value::Uint64(5000))])
        } else {
            Value::Object(vec![("releaseCredits".into(), Value::Uint64(0))])
        };
        if factors == &expected {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
    // Original request IDs and complete payload/hash remain independently bound when output arrives on a retry stream.
    fn validate_result(&self, _: &Invocation, input: &Value, output: &Value) -> Result<(), Error> {
        if field(input, "operation")? != field(output, "operation")?
            || field(input, "placementId")? != field(output, "placementId")?
        {
            return Err(Error::Invalid);
        }
        let id = if field(input, "operation")? == &Value::String("recover".into()) {
            "recoveryId"
        } else {
            "removalId"
        };
        if field(input, id)? != field(output, id)? {
            return Err(Error::Invalid);
        }
        if id == "recoveryId" {
            use sha2::{Digest, Sha256};
            let Value::Bytes(data) = field(output, "payload")? else {
                return Err(Error::Invalid);
            };
            if field(input, "payloadHash")? != &Value::Bytes(Sha256::digest(data).to_vec())
                || data.len() != 5000
            {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }
    // This fixture does not implement the optional Messaging private byte-check resource.
    fn verify(&self, _: &Value, _: &Blob, _: &Value, _: Option<&Value>) -> Result<Check, Error> {
        Ok(Check::Inconclusive)
    }
}
impl Provider for Messaging {
    // Retained original owner data is the only cache authority; current balance and pending-state checks are deliberately absent.
    fn allow_cached_retry(
        &self,
        original: &Invocation,
        peer: Peer,
        input: &Value,
    ) -> Result<bool, Error> {
        self.cache_reads.fetch_add(1, Ordering::SeqCst);
        if original.requester != peer.remote || self.cached_retry(input)?.is_none() {
            return Err(Error::Denied);
        }
        Ok(self.allow.load(Ordering::SeqCst))
    }
    // Ordinary preacceptance assessment is counted separately from replay and exposes only fixed provider factors.
    fn assess(&self, _: Peer, _: &Offering, input: &Value) -> Result<Assessment, Error> {
        let factors = if field(input, "operation")? == &Value::String("recover".into()) {
            Value::Object(vec![("payloadSize".into(), Value::Uint64(5000))])
        } else {
            Value::Object(vec![("releaseCredits".into(), Value::Uint64(0))])
        };
        Ok(Assessment {
            metadata: input.clone(),
            factors,
            input_bytes: input.byte_len(Budget {
                depth: 31,
                nodes: 65536,
                bytes: 16_777_216,
            })? as u64,
            pin: None,
        })
    }
    // One real coordinator dispatch stages a test-owned effect; complete normative-shaped output is retained independently of sessions.
    fn execute(&self, i: &Invocation, input: &Value) -> Result<Completion, Error> {
        let staged = self
            .effects
            .execute(i, &Value::String("fixture replay effect".into()))?;
        let Value::Object(mut fields) = input.clone() else {
            return Err(Error::Invalid);
        };
        let remove = field(input, "operation")? == &Value::String("remove".into());
        if remove {
            fields.retain(|(n, _)| {
                !matches!(n.as_str(), "expectedExpiresAt" | "minimumReleaseCredits")
            });
            fields.extend([
                ("removed".into(), Value::Bool(true)),
                ("releaseCredits".into(), Value::Uint64(0)),
                ("price".into(), Value::Uint64(7)),
            ]);
        } else {
            fields.extend([
                ("senderId".into(), Value::Bytes(vec![9; 32])),
                ("recipientId".into(), Value::Bytes(i.requester.0.to_vec())),
                ("senderCreatedAt".into(), Value::Uint64(1)),
                ("payloadEncoding".into(), Value::String("opaque".into())),
                ("payloadSize".into(), Value::Uint64(5000)),
                ("senderPublicKey".into(), Value::Bytes(vec![8; 32])),
                ("senderSignature".into(), Value::Bytes(vec![8; 64])),
                ("payload".into(), Value::Bytes(payload())),
                ("recoveredAt".into(), Value::Uint64(1)),
                ("providerSignature".into(), Value::Bytes(vec![8; 64])),
            ]);
        }
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        let output = Value::Object(fields);
        let result = services::codec::encode(
            &output,
            Budget {
                depth: 31,
                nodes: 65536,
                bytes: 16_777_216,
            },
            16_777_216,
        )?;
        let completion = Completion {
            output,
            result: Blob::new(result, 16_777_216)?,
            price: 7,
            effect: staged.effect,
        };
        self.results
            .lock()
            .unwrap()
            .insert(i.key, completion.clone());
        Ok(completion)
    }
    // Recovery never repeats execution; the complete cached result is an immutable original-key observation.
    fn completion(&self, key: OperationKey) -> Result<Completion, Error> {
        self.results
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .ok_or(Error::Unknown)
    }
    // Participant staging/decision delivery uses the real fixture's exact owner/effect token behavior.
    fn prepare(&self, key: OperationKey, effect: &Blob) -> Result<Blob, Error> {
        self.effects.prepare(key, effect)
    }
    // Commit acknowledgements are idempotent and retain the original effect.
    fn commit(&self, key: OperationKey, token: &Blob) -> Result<(), Error> {
        self.effects.commit(key, token)
    }
    // An abort cannot undo a committed effect.
    fn abort(&self, key: OperationKey, token: Option<&Blob>) -> Result<(), Error> {
        self.effects.abort(key, token)
    }
    // Status remains a local participant fact, not a remote job-status operation.
    fn status(&self, key: OperationKey) -> Result<Status, Error> {
        self.effects.status(key)
    }
    // No fake remote byte-check service is enabled by the replay fixture.
    fn evidence(&self, _: &services::checks::Invocation, _: &Value) -> Result<Value, Error> {
        Err(Error::Unsupported)
    }
}
