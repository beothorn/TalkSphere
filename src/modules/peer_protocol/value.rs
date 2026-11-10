//! Schema-bound metadata, deterministic descriptor assignment and bounded unresolved body ownership.
use super::{Error, contract::Limits, inline, scalar};
use crate::modules::services::{
    handler::Blob,
    schema::{self, Support},
    value::{Budget, Value},
};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
/// Immutable transfer declaration; body IDs follow metadata traversal order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    /// Sequential uint32 body ID.
    pub id: u32,
    /// Exact logical byte length, positive and locally bounded.
    pub length: u64,
    /// Whether complete bytes must decode as UTF-8 text.
    pub text: bool,
}
/// Outgoing metadata and immutable source bodies in increasing ID order.
pub struct Encoded {
    /// Wire metadata, independently bounded by the enclosing control encoder.
    pub metadata: Json,
    /// Exact byte sources; vector index is the assigned body ID.
    pub bodies: Vec<Blob>,
}
/// Validated provisional metadata; no complete application Value exists until bodies are assembled.
pub struct Pending {
    root: Node,
    descriptors: Vec<Descriptor>,
    limits: Limits,
}
enum Node {
    Scalar(Value),
    Object(Vec<(String, Node)>),
    Array(Vec<Node>),
    Body(u32, bool),
}
impl Pending {
    /// Parse metadata using the retrieved known schema and independent logical resource limits.
    /// # Errors
    /// Malformed values/descriptors, wrong IDs, unsupported schemas and exhausted budgets fail before body allocation.
    pub fn parse(
        metadata: &Json,
        descriptor: &Json,
        limits: Limits,
        nonfinite: bool,
    ) -> Result<Self, Error> {
        tracing::trace!("Validating schema-bound provisional peer metadata");
        limits.validate()?;
        match schema::validate(descriptor).map_err(|e| match e {
            schema::Error::Invalid => Error::Invalid,
            schema::Error::Limit => Error::Limit,
        })? {
            Support::Known => {}
            Support::Unknown => return Err(Error::Unsupported),
        }
        let mut builder = Builder {
            limits,
            nodes: limits.nodes,
            bytes: limits.state_bytes,
            descriptors: vec![],
        };
        let root = builder
            .read(metadata, Some(descriptor), nonfinite, 0)?
            .ok_or(Error::Invalid)?;
        Ok(Self {
            root,
            descriptors: builder.descriptors,
            limits,
        })
    }
    /// Borrow exact declared body identities, lengths and text requirements without allocating their payloads.
    pub fn descriptors(&self) -> &[Descriptor] {
        tracing::trace!("Reading bounded peer body declarations");
        &self.descriptors
    }
    /// Assemble only complete exact bodies; ignored extension bodies are consumed without application semantics.
    /// # Errors
    /// Missing/extra IDs, length mismatch, invalid final UTF-8 and exhausted complete-value budgets fail.
    pub fn assemble(&self, bodies: &BTreeMap<u32, Blob>) -> Result<Value, Error> {
        tracing::trace!("Assembling complete schema-bound peer value");
        if bodies.len() != self.descriptors.len() {
            return Err(Error::Invalid);
        }
        for d in &self.descriptors {
            let b = bodies.get(&d.id).ok_or(Error::Invalid)?;
            if b.bytes().len() as u64 != d.length {
                return Err(Error::Invalid);
            }
            if d.text {
                std::str::from_utf8(b.bytes()).map_err(|_| Error::Invalid)?;
            }
        }
        let value = self.root.assemble(bodies)?;
        value.bounded(budget(self.limits))?;
        Ok(value)
    }
}
impl Node {
    // Materialize the known logical tree only after every body and UTF-8 requirement has passed.
    fn assemble(&self, bodies: &BTreeMap<u32, Blob>) -> Result<Value, Error> {
        match self {
            Self::Scalar(value) => Ok(value.clone()),
            Self::Object(fields) => Ok(Value::Object(
                fields
                    .iter()
                    .map(|(name, node)| Ok((name.clone(), node.assemble(bodies)?)))
                    .collect::<Result<_, Error>>()?,
            )),
            Self::Array(items) => Ok(Value::Array(
                items
                    .iter()
                    .map(|node| node.assemble(bodies))
                    .collect::<Result<_, _>>()?,
            )),
            Self::Body(id, text) => {
                let bytes = bodies.get(id).ok_or(Error::Invalid)?.bytes();
                if *text {
                    Ok(Value::String(
                        std::str::from_utf8(bytes)
                            .map_err(|_| Error::Invalid)?
                            .to_owned(),
                    ))
                } else {
                    Ok(Value::Bytes(bytes.to_vec()))
                }
            }
        }
    }
}
/// Encode typed values with deterministic UTF-8 field ordering and bodies for large byte/text values.
/// The caller separately validates the installed service schema before proposing or returning work.
/// # Errors
/// Structural limits, unsupported values and forbidden exceptional floats fail before an Encoded value is returned.
pub fn encode(value: &Value, limits: Limits, nonfinite: bool) -> Result<Encoded, Error> {
    tracing::trace!("Binding typed peer values and deterministic detached bodies");
    limits.validate()?;
    value.bounded(budget(limits))?;
    let mut bodies = vec![];
    let metadata = encode_node(value, limits, nonfinite, &mut bodies)?;
    Ok(Encoded { metadata, bodies })
}
// Preserve logical byte accounting independently of metadata wrapper overhead, which the control layer bounds.
fn budget(limits: Limits) -> Budget {
    Budget {
        depth: 31,
        nodes: limits.nodes,
        bytes: limits.state_bytes,
    }
}
// Visit names by unsigned UTF-8 bytes and arrays by index so descriptor IDs are independent of insertion order.
fn encode_node(
    value: &Value,
    limits: Limits,
    nonfinite: bool,
    bodies: &mut Vec<Blob>,
) -> Result<Json, Error> {
    match value {
        Value::Object(fields) => {
            let mut fields = fields.iter().collect::<Vec<_>>();
            fields.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            let mut map = serde_json::Map::new();
            for (name, value) in fields {
                map.insert(name.clone(), encode_node(value, limits, nonfinite, bodies)?);
            }
            Ok(Json::Object(map))
        }
        Value::Array(items) => Ok(Json::Array(
            items
                .iter()
                .map(|v| encode_node(v, limits, nonfinite, bodies))
                .collect::<Result<_, _>>()?,
        )),
        Value::Bytes(bytes) if bytes.len() <= inline::INLINE_MAX => inline::encode(bytes),
        Value::Bytes(bytes) => detach(bytes, false, limits, bodies),
        Value::String(text) if text.len() > inline::INLINE_MAX => {
            detach(text.as_bytes(), true, limits, bodies)
        }
        value => scalar::encode(value, nonfinite),
    }
}
// Retain each complete immutable source only after body-count and logical byte admission.
fn detach(bytes: &[u8], text: bool, limits: Limits, bodies: &mut Vec<Blob>) -> Result<Json, Error> {
    tracing::trace!("Assigning one bounded detached peer body");
    if bodies.len() >= limits.bodies {
        return Err(Error::Limit);
    }
    let id = u32::try_from(bodies.len()).map_err(|_| Error::Limit)?;
    let value = if text {
        json!({"$text":id,"length":bytes.len().to_string()})
    } else {
        json!({"$bytes":id,"length":bytes.len().to_string()})
    };
    bodies.push(Blob::new(bytes.to_vec(), limits.state_bytes).map_err(|_| Error::Limit)?);
    Ok(value)
}
struct Builder {
    limits: Limits,
    nodes: usize,
    bytes: usize,
    descriptors: Vec<Descriptor>,
}
impl Builder {
    // Bound every known or ignored logical node; reserved descriptors remain transport obligations even in ignored extensions.
    fn read(
        &mut self,
        value: &Json,
        schema: Option<&Json>,
        nonfinite: bool,
        depth: usize,
    ) -> Result<Option<Node>, Error> {
        if depth > 31 {
            return Err(Error::Limit);
        }
        self.nodes = self.nodes.checked_sub(1).ok_or(Error::Limit)?;
        let kind = schema.and_then(|s| s["type"].as_str());
        if let Some(map) = value.as_object() {
            let has_bytes = map.contains_key("$bytes");
            let has_text = map.contains_key("$text");
            if matches!(kind, Some("bytes" | "string") | None) && (has_bytes || has_text) {
                if map.len() != 2
                    || has_bytes == has_text
                    || !map.contains_key("length")
                    || matches!(kind, Some("bytes")) && has_text
                    || matches!(kind, Some("string")) && has_bytes
                {
                    return Err(Error::Invalid);
                }
                let id = scalar::uint32(&map[if has_text { "$text" } else { "$bytes" }])?;
                let length = scalar::uint64(&map["length"])?;
                if id as usize != self.descriptors.len() || length == 0 {
                    return Err(Error::Invalid);
                }
                if self.descriptors.len() >= self.limits.bodies {
                    return Err(Error::Limit);
                }
                self.charge(usize::try_from(length).map_err(|_| Error::Limit)?)?;
                self.descriptors.push(Descriptor {
                    id,
                    length,
                    text: has_text,
                });
                return Ok(kind.map(|_| Node::Body(id, has_text)));
            }
        }
        match kind {
            Some("object") => {
                let values = value.as_object().ok_or(Error::Invalid)?;
                let fields = schema.ok_or(Error::Invalid)?["fields"]
                    .as_object()
                    .ok_or(Error::Invalid)?;
                for (name, schema) in fields {
                    if !values.contains_key(name)
                        && schema.get("optional") != Some(&Json::Bool(true))
                    {
                        return Err(Error::Invalid);
                    }
                }
                let mut result = vec![];
                let mut values = values.iter().collect::<Vec<_>>();
                values.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
                for (name, value) in values {
                    self.charge(name.len())?;
                    if let Some(node) = self.read(value, fields.get(name), nonfinite, depth + 1)? {
                        result.push((name.clone(), node));
                    }
                }
                Ok(Some(Node::Object(result)))
            }
            Some("array") => {
                let values = value.as_array().ok_or(Error::Invalid)?;
                let item = &schema.ok_or(Error::Invalid)?["items"];
                let mut result = vec![];
                for value in values {
                    result.push(
                        self.read(value, Some(item), nonfinite, depth + 1)?
                            .ok_or(Error::Invalid)?,
                    );
                }
                Ok(Some(Node::Array(result)))
            }
            Some("bytes") => {
                let bytes = inline::decode(value)?;
                self.charge(bytes.len())?;
                Ok(Some(Node::Scalar(Value::Bytes(bytes))))
            }
            Some(kind) => {
                let v = scalar::decode(kind, value, nonfinite)?;
                if let Value::String(text) = &v {
                    self.charge(text.len())?;
                }
                Ok(Some(Node::Scalar(v)))
            }
            None => {
                match value {
                    Json::Object(fields) => {
                        let mut fields = fields.iter().collect::<Vec<_>>();
                        fields.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
                        for (name, value) in fields {
                            self.charge(name.len())?;
                            self.read(value, None, nonfinite, depth + 1)?;
                        }
                    }
                    Json::Array(items) => {
                        for value in items {
                            self.read(value, None, nonfinite, depth + 1)?;
                        }
                    }
                    Json::String(s) => self.charge(s.len())?,
                    _ => {}
                }
                Ok(None)
            }
        }
    }
    // Checked subtraction reserves declared detached length before accepting any body bytes.
    fn charge(&mut self, bytes: usize) -> Result<(), Error> {
        self.bytes = self.bytes.checked_sub(bytes).ok_or(Error::Limit)?;
        Ok(())
    }
}
