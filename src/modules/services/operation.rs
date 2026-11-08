//! Private immutable invocation binding and recoverable coordinator lifecycle image.
use super::{
    Error, codec,
    contract::{Limits, OperationKey},
    handler::{Blob, Completion},
    pricing::Price,
    value::{Budget, Value},
};
use crate::modules::{identity::api::IdentityId, ledger::terms::Terms};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Durable local invocation lifecycle, independent of payment and remote receipt.
pub enum Phase {
    /// Immutable consent is durable; reservation may need original-key recovery.
    Accepted,
    /// Finite local capacity is reserved; execution has not begun.
    Reserved,
    /// The execution attempt is durable; recovery must not rerun it.
    Executing,
    /// Complete validated result and staged effect are retained.
    Staged,
    /// All required local participants have prepared exact effects.
    Prepared,
    /// An immutable commit decision requires eventual delivery.
    Commit,
    /// An immutable abort decision requires eventual delivery.
    Abort,
    /// All local effects and retained result are durable.
    Committed,
    /// All uncommitted resources have been released without charge.
    Aborted,
}
#[derive(Clone)]
pub(super) struct Operation {
    pub key: OperationKey,
    pub service: String,
    pub version: u32,
    pub terms: Terms,
    pub definition: Vec<u8>,
    pub metadata: Value,
    pub pricing: Price,
    pub input_bytes: u64,
    pub factors: Value,
    pub phase: Phase,
    pub input: Option<Value>,
    pub completion: Option<Completion>,
    pub handler: Option<Blob>,
}
// Shared local image budgets include opaque result bytes; decoding is independent of peer JSON.
fn budget(l: Limits) -> Budget {
    Budget {
        depth: 31,
        nodes: l.nodes,
        bytes: l.state_bytes,
    }
}
// Encode a whole versioned image in deterministic operation-key order, never an append-only partial record.
pub(super) fn encode(
    records: &std::collections::BTreeMap<OperationKey, Operation>,
    l: Limits,
) -> Result<Vec<u8>, Error> {
    if records.len() > l.operations {
        return Err(Error::Limit);
    }
    let values = records
        .values()
        .map(Operation::value)
        .collect::<Result<Vec<_>, _>>()?;
    codec::encode(
        &Value::Array(vec![Value::Uint32(1), Value::Array(values)]),
        budget(l),
        l.state_bytes,
    )
}
// Reject foreign versions, duplicate operation keys and inconsistent effect/decision records before recovery.
pub(super) fn decode(
    bytes: &[u8],
    l: Limits,
) -> Result<std::collections::BTreeMap<OperationKey, Operation>, Error> {
    let root = codec::decode(bytes, budget(l), l.state_bytes)?;
    let Value::Array(root) = root else {
        return Err(Error::Invalid);
    };
    let [Value::Uint32(1), Value::Array(values)] = root.as_slice() else {
        return Err(Error::Unsupported);
    };
    if values.len() > l.operations {
        return Err(Error::Limit);
    }
    let mut records = std::collections::BTreeMap::new();
    for value in values {
        let o = Operation::parse(value)?;
        if records.insert(o.key, o).is_some() {
            return Err(Error::Invalid);
        }
    }
    Ok(records)
}
impl Operation {
    // Reconstruct explicit public handler contracts without exposing this private record representation.
    pub(super) fn invocation(&self, limits: Limits) -> Result<super::handler::Invocation, Error> {
        tracing::trace!("Reconstructing exact public invocation contract from retained binding");
        let value = codec::decode(&self.terms.accepted, budget(limits), limits.state_bytes)?;
        let Value::Array(fields) = value else {
            return Err(Error::Invalid);
        };
        if fields.len() != 5 || bytes(&fields[1])? != self.definition {
            return Err(Error::Invalid);
        }
        let publisher = IdentityId(fixed(&fields[3])?);
        let wanted = boolean(&fields[4])?;
        let source = crate::modules::board::document::Document::parse(
            &bytes(&fields[2])?,
            Default::default(),
        )
        .map_err(|_| Error::Invalid)?;
        let root = if wanted {
            serde_json::json!({"version":1,"providedServices":[],"wantedServices":[source.value()]})
        } else {
            serde_json::json!({"version":1,"providedServices":[source.value()],"wantedServices":[]})
        };
        let board = crate::modules::board::offering::Board::parse(
            root.to_string().as_bytes(),
            Default::default(),
        )
        .map_err(|_| Error::Invalid)?;
        let offering = board.offerings.into_values().next().ok_or(Error::Invalid)?;
        if offering.service != self.service
            || offering.version != self.version
            || offering.issuer != self.terms.issuer
            || (wanted && publisher != self.terms.requester)
            || (!wanted && publisher != self.terms.provider)
        {
            return Err(Error::Invalid);
        }
        let definition = crate::modules::board::definition::Definition::parse(
            &self.definition,
            &offering,
            Default::default(),
        )
        .map_err(|_| Error::Invalid)?;
        let quote = Blob::new(bytes(&fields[0])?, limits.state_bytes)?;
        if quote.bytes().is_empty() {
            return Err(Error::Invalid);
        }
        Ok(super::handler::Invocation {
            key: self.key,
            requester: self.terms.requester,
            provider: self.terms.provider,
            accepted: Blob::new(self.terms.accepted.clone(), limits.state_bytes)?,
            maximum: self.terms.maximum,
            publisher,
            issuer: self.terms.issuer,
            offering,
            definition,
            quote,
            factors: self.factors.clone(),
            pricing: self.pricing,
        })
    }
    // Bind the exact retained input, output, bytes and accepted terms without making Ledger own family payloads.
    pub(super) fn reference(&self, limits: Limits) -> Result<Vec<u8>, Error> {
        use sha2::{Digest, Sha256};
        tracing::trace!("Binding retained Services result and exact settlement effect");
        let c = self.completion.as_ref().ok_or(Error::Invalid)?;
        let value = Value::Array(vec![
            self.input.clone().ok_or(Error::Invalid)?,
            c.output.clone(),
            Value::Bytes(c.result.bytes().to_vec()),
            Value::Bytes(c.effect.bytes().to_vec()),
            Value::Uint64(c.price),
            Value::Bytes(self.terms.accepted.clone()),
            self.factors.clone(),
        ]);
        let bytes = codec::encode(&value, budget(limits), limits.state_bytes)?;
        let mut hash = Sha256::new();
        hash.update(b"TalkSphere Services retained result v1");
        hash.update(self.key.requester.0);
        hash.update(self.key.connection);
        hash.update(self.key.stream.to_be_bytes());
        hash.update(bytes);
        Ok(hash.finalize().to_vec())
    }
    // Serialize every immutable binding, retained input/result and preparation token explicitly.
    fn value(&self) -> Result<Value, Error> {
        let price = match self.pricing {
            Price::Fixed(n) => vec![Value::Uint64(n)],
            Price::Computation {
                minimum,
                numerator,
                denominator,
                milliseconds,
            } => vec![
                Value::Uint64(minimum),
                Value::Uint64(numerator),
                Value::Uint64(denominator),
                Value::Uint64(milliseconds),
            ],
        };
        let completion = self.completion.as_ref().map(|c| {
            Value::Array(vec![
                c.output.clone(),
                Value::Bytes(c.result.bytes().to_vec()),
                Value::Uint64(c.price),
                Value::Bytes(c.effect.bytes().to_vec()),
            ])
        });
        let t = &self.terms;
        Ok(Value::Array(vec![
            Value::Bytes(self.key.requester.0.to_vec()),
            Value::Bytes(self.key.connection.to_vec()),
            Value::Uint32(self.key.stream),
            Value::String(self.service.clone()),
            Value::Uint32(self.version),
            Value::Bytes(t.provider.0.to_vec()),
            Value::Bytes(t.issuer.0.to_vec()),
            Value::Uint64(t.maximum),
            Value::Bool(t.fixed),
            Value::Bytes(t.accepted.clone()),
            optional(t.transfer.map(|b| Value::Bytes(b.to_vec()))),
            optional(t.fee.map(Value::Uint64)),
            Value::Uint64(t.spending_bound),
            Value::Bytes(self.definition.clone()),
            self.metadata.clone(),
            Value::Array(price),
            Value::Uint32(self.phase as u32),
            optional(self.input.clone()),
            optional(completion),
            optional(
                self.handler
                    .as_ref()
                    .map(|b| Value::Bytes(b.bytes().to_vec())),
            ),
            Value::Uint64(self.input_bytes),
            self.factors.clone(),
        ]))
    }
    // Require exact field count/kinds and revalidate immutable amount/role/phase associations on restart.
    fn parse(value: &Value) -> Result<Self, Error> {
        let Value::Array(v) = value else {
            return Err(Error::Invalid);
        };
        if v.len() != 22 {
            return Err(Error::Invalid);
        }
        let key = OperationKey {
            requester: IdentityId(fixed(&v[0])?),
            connection: fixed(&v[1])?,
            stream: u32_value(&v[2])?,
        };
        let service = string(&v[3])?;
        let version = u32_value(&v[4])?;
        let terms = Terms {
            requester: key.requester,
            provider: IdentityId(fixed(&v[5])?),
            issuer: IdentityId(fixed(&v[6])?),
            maximum: amount(&v[7])?,
            fixed: boolean(&v[8])?,
            accepted: bytes(&v[9])?,
            transfer: some(&v[10])?.map(fixed).transpose()?,
            fee: some(&v[11])?.map(amount).transpose()?,
            spending_bound: amount(&v[12])?,
        };
        let Value::Array(p) = &v[15] else {
            return Err(Error::Invalid);
        };
        let pricing = match p.as_slice() {
            [n] => Price::Fixed(amount(n)?),
            [m, n, d, t] => Price::Computation {
                minimum: amount(m)?,
                numerator: amount(n)?,
                denominator: amount(d)?,
                milliseconds: amount(t)?,
            },
            _ => return Err(Error::Invalid),
        };
        let phase = match u32_value(&v[16])? {
            0 => Phase::Accepted,
            1 => Phase::Reserved,
            2 => Phase::Executing,
            3 => Phase::Staged,
            4 => Phase::Prepared,
            5 => Phase::Commit,
            6 => Phase::Abort,
            7 => Phase::Committed,
            8 => Phase::Aborted,
            _ => return Err(Error::Invalid),
        };
        let completion = some(&v[18])?
            .map(|v| {
                let Value::Array(c) = v else {
                    return Err(Error::Invalid);
                };
                if c.len() != 4 {
                    return Err(Error::Invalid);
                }
                Ok(Completion {
                    output: c[0].clone(),
                    result: Blob::new(bytes(&c[1])?, usize::MAX)?,
                    price: amount(&c[2])?,
                    effect: Blob::new(bytes(&c[3])?, usize::MAX)?,
                })
            })
            .transpose()?;
        let o = Self {
            key,
            service,
            version,
            terms,
            definition: bytes(&v[13])?,
            metadata: v[14].clone(),
            pricing,
            phase,
            input_bytes: amount(&v[20])?,
            factors: v[21].clone(),
            input: some(&v[17])?.cloned(),
            completion,
            handler: some(&v[19])?
                .map(|v| Blob::new(bytes(v)?, usize::MAX))
                .transpose()?,
        };
        if key.stream == 0
            || o.service.is_empty()
            || version == 0
            || o.terms.accepted.is_empty()
            || o.definition.is_empty()
            || o.pricing.maximum(&o.service, version)? != o.terms.maximum
            || o.terms.fixed != matches!(pricing, Price::Fixed(_))
            || o.terms
                .maximum
                .checked_add(o.terms.fee.unwrap_or(0))
                .ok_or(Error::Limit)?
                > o.terms.spending_bound
        {
            return Err(Error::Invalid);
        }
        if matches!(
            phase,
            Phase::Executing | Phase::Staged | Phase::Prepared | Phase::Commit | Phase::Committed
        ) && o.input.is_none()
        {
            return Err(Error::Invalid);
        }
        if matches!(
            phase,
            Phase::Staged | Phase::Prepared | Phase::Commit | Phase::Committed
        ) && o.completion.is_none()
        {
            return Err(Error::Invalid);
        }
        if matches!(phase, Phase::Prepared | Phase::Commit | Phase::Committed)
            && o.handler.is_none()
        {
            return Err(Error::Invalid);
        }
        if let Some(c) = &o.completion
            && (c.price == 0
                || c.price > o.terms.maximum
                || (o.terms.fixed && c.price != o.terms.maximum)
                || c.effect.bytes().is_empty())
        {
            return Err(Error::Invalid);
        }
        Ok(o)
    }
}
// Encode option presence distinctly from logical omission or null.
fn optional(v: Option<Value>) -> Value {
    Value::Array(v.into_iter().collect())
}
// Decode only the canonical zero-or-one option representation.
fn some(v: &Value) -> Result<Option<&Value>, Error> {
    match v {
        Value::Array(v) if v.len() <= 1 => Ok(v.first()),
        _ => Err(Error::Invalid),
    }
}
// Exact local scalar helpers reject coercion and malformed serialized records.
fn amount(v: &Value) -> Result<u64, Error> {
    if let Value::Uint64(n) = v {
        Ok(*n)
    } else {
        Err(Error::Invalid)
    }
}
// Version/stream/phase fields must preserve their explicit uint32 width.
fn u32_value(v: &Value) -> Result<u32, Error> {
    if let Value::Uint32(n) = v {
        Ok(*n)
    } else {
        Err(Error::Invalid)
    }
}
// Copy only already bounded local byte fields after the image codec has admitted them.
fn bytes(v: &Value) -> Result<Vec<u8>, Error> {
    if let Value::Bytes(b) = v {
        Ok(b.clone())
    } else {
        Err(Error::Invalid)
    }
}
// Exact identity and transfer fields cannot truncate or pad malformed local data.
fn fixed<const N: usize>(v: &Value) -> Result<[u8; N], Error> {
    bytes(v)?.try_into().map_err(|_| Error::Invalid)
}
// Textual contract names retain exact UTF-8 without normalization.
fn string(v: &Value) -> Result<String, Error> {
    if let Value::String(s) = v {
        Ok(s.clone())
    } else {
        Err(Error::Invalid)
    }
}
// Fixed/metered markers cannot use numeric truthiness.
fn boolean(v: &Value) -> Result<bool, Error> {
    if let Value::Bool(b) = v {
        Ok(*b)
    } else {
        Err(Error::Invalid)
    }
}
