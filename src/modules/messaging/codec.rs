//! Decoding and encoding of Messaging operations between typed values and the Services logical value model.
//!
//! Callers: the provider (decode requests, encode responses) and the requester (encode requests, decode
//! responses). Every field has its exact logical type and fixed length; a field that belongs only to another
//! operation is rejected, an unknown field follows the compatible-extension rule (ignored), duplicate names
//! and wrong types are invalid, and nothing here trusts a peer-supplied size before the payload length is
//! compared with it. Wire encoding of these values belongs to Peer Protocol.
use super::{
    error::Error,
    ids::{MessageId, PayloadHash, PlacementId, RecoveryId, RemovalId, Signature},
    metadata::{Metadata, SignedMessage},
    operation::{
        CheckInput, CheckOutput, Cursor, DiscoverInput, DiscoverOutput, Entry, Receipt,
        RecoverInput, RecoverOutput, RemoveInput, RemoveOutput, RenewInput, RenewOutput,
        StatusInput, StatusOutput, StoreInput, StoreOutput,
    },
    state::{Encoding, State},
};
use crate::modules::{identity::api::IdentityId, services::value::Value};

/// A decoded request of one of the six operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Retain a message.
    Store(StoreInput),
    /// List pending placements.
    Discover(DiscoverInput),
    /// Recover a message.
    Recover(RecoverInput),
    /// Ask about a placement.
    Status(StatusInput),
    /// Extend retention.
    Renew(RenewInput),
    /// End a placement early.
    Remove(RemoveInput),
}

/// A decoded response of one of the six operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// Result of a store.
    Store(StoreOutput),
    /// One page of discovery.
    Discover(DiscoverOutput),
    /// A recovered message and its receipt.
    Recover(RecoverOutput),
    /// The sender's status.
    Status(StatusOutput),
    /// Result of a renewal.
    Renew(RenewOutput),
    /// Result of a removal.
    Remove(RemoveOutput),
}

// Every field name any operation input uses, so a field of another operation can be recognized and refused.
const KNOWN_INPUT: [&str; 21] = [
    "operation",
    "placementId",
    "messageId",
    "senderId",
    "recipientId",
    "senderCreatedAt",
    "payloadEncoding",
    "payloadSize",
    "payloadHash",
    "senderPublicKey",
    "senderSignature",
    "payload",
    "retentionSeconds",
    "afterAcceptedAt",
    "afterPlacementId",
    "limit",
    "recoveryId",
    "recipientPublicKey",
    "recipientSignature",
    "expectedExpiresAt",
    "additionalSeconds",
];
const KNOWN_INPUT_MORE: [&str; 2] = ["removalId", "minimumReleaseCredits"];
const STORE_FIELDS: [&str; 11] = [
    "messageId",
    "senderId",
    "recipientId",
    "senderCreatedAt",
    "payloadEncoding",
    "payloadSize",
    "payloadHash",
    "senderPublicKey",
    "senderSignature",
    "retentionSeconds",
    "payload",
];
const DISCOVER_FIELDS: [&str; 3] = ["afterAcceptedAt", "afterPlacementId", "limit"];
const RECOVER_FIELDS: [&str; 6] = [
    "placementId",
    "messageId",
    "payloadHash",
    "recoveryId",
    "recipientPublicKey",
    "recipientSignature",
];
const STATUS_FIELDS: [&str; 1] = ["placementId"];
const RENEW_FIELDS: [&str; 3] = ["placementId", "expectedExpiresAt", "additionalSeconds"];
const REMOVE_FIELDS: [&str; 4] = [
    "placementId",
    "removalId",
    "expectedExpiresAt",
    "minimumReleaseCredits",
];

/// Decode a request value.
/// # Errors
/// `Invalid` for a non-object, duplicate names, a missing or wrongly typed or sized field, a field of another operation, a
/// single cursor field or a zero limit or payload; `Unsupported` for an unknown operation or payload encoding.
pub fn parse_request(value: &Value) -> Result<Request, Error> {
    tracing::trace!("Decoding a Messaging request");
    let fields = Fields::new(value)?;
    let operation = fields.text("operation")?;
    let allowed: &[&str] = match operation {
        "store" => &STORE_FIELDS,
        "discover" => &DISCOVER_FIELDS,
        "recover" => &RECOVER_FIELDS,
        "status" => &STATUS_FIELDS,
        "renew" => &RENEW_FIELDS,
        "remove" => &REMOVE_FIELDS,
        _ => return Err(Error::Unsupported),
    };
    for (name, _) in fields.0 {
        let known =
            KNOWN_INPUT.contains(&name.as_str()) || KNOWN_INPUT_MORE.contains(&name.as_str());
        if known && name != "operation" && !allowed.contains(&name.as_str()) {
            return Err(Error::Invalid);
        }
    }
    Ok(match operation {
        "store" => Request::Store(parse_store(&fields)?),
        "discover" => Request::Discover(parse_discover(&fields)?),
        "recover" => Request::Recover(RecoverInput {
            placement_id: PlacementId(fields.array("placementId")?),
            message_id: MessageId(fields.array("messageId")?),
            payload_hash: PayloadHash(fields.array("payloadHash")?),
            recovery_id: RecoveryId(fields.array("recoveryId")?),
            recipient_public_key: fields.array("recipientPublicKey")?,
            recipient_signature: Signature(fields.array("recipientSignature")?),
        }),
        "status" => Request::Status(StatusInput {
            placement_id: PlacementId(fields.array("placementId")?),
        }),
        "renew" => Request::Renew(RenewInput {
            placement_id: PlacementId(fields.array("placementId")?),
            expected_expires_at: fields.u64("expectedExpiresAt")?,
            additional_seconds: fields.u64("additionalSeconds")?,
        }),
        _ => Request::Remove(RemoveInput {
            placement_id: PlacementId(fields.array("placementId")?),
            removal_id: RemovalId(fields.array("removalId")?),
            expected_expires_at: fields.u64("expectedExpiresAt")?,
            minimum_release_credits: fields.u64("minimumReleaseCredits")?,
        }),
    })
}

// A store request: complete signed message, retention and an exactly matching payload size.
fn parse_store(fields: &Fields) -> Result<StoreInput, Error> {
    let encoding = Encoding::parse(fields.text("payloadEncoding")?).ok_or(Error::Unsupported)?;
    let payload = fields.bytes("payload")?.to_vec();
    let payload_size = fields.u64("payloadSize")?;
    if payload_size == 0 || payload.len() as u64 != payload_size {
        return Err(Error::Invalid);
    }
    Ok(StoreInput {
        message: SignedMessage {
            metadata: Metadata {
                message_id: MessageId(fields.array("messageId")?),
                sender_id: IdentityId(fields.array("senderId")?),
                recipient_id: IdentityId(fields.array("recipientId")?),
                sender_created_at: fields.u64("senderCreatedAt")?,
                encoding,
                payload_size,
                payload_hash: PayloadHash(fields.array("payloadHash")?),
            },
            sender_public_key: fields.array("senderPublicKey")?,
            sender_signature: Signature(fields.array("senderSignature")?),
            payload,
        },
        retention_seconds: fields.u64("retentionSeconds")?,
    })
}

// A discovery request: a positive limit and either both cursor fields or neither.
fn parse_discover(fields: &Fields) -> Result<DiscoverInput, Error> {
    let limit = fields.u32("limit")?;
    if limit == 0 {
        return Err(Error::Invalid);
    }
    let cursor = match (
        fields.has("afterAcceptedAt"),
        fields.has("afterPlacementId"),
    ) {
        (false, false) => None,
        (true, true) => Some(Cursor {
            after_accepted_at: fields.u64("afterAcceptedAt")?,
            after_placement_id: PlacementId(fields.array("afterPlacementId")?),
        }),
        _ => return Err(Error::Invalid),
    };
    Ok(DiscoverInput { cursor, limit })
}

/// Encode a request as the logical value a requester sends.
pub fn request_value(request: &Request) -> Value {
    tracing::trace!("Encoding a Messaging request");
    let mut f = Out::new();
    match request {
        Request::Store(r) => {
            let m = &r.message.metadata;
            f.text("operation", "store");
            f.bytes("messageId", &m.message_id.0);
            f.bytes("senderId", &m.sender_id.0);
            f.bytes("recipientId", &m.recipient_id.0);
            f.u64("senderCreatedAt", m.sender_created_at);
            f.text("payloadEncoding", m.encoding.name());
            f.u64("payloadSize", m.payload_size);
            f.bytes("payloadHash", &m.payload_hash.0);
            f.bytes("senderPublicKey", &r.message.sender_public_key);
            f.bytes("senderSignature", &r.message.sender_signature.0);
            f.u64("retentionSeconds", r.retention_seconds);
            f.bytes("payload", &r.message.payload);
        }
        Request::Discover(r) => {
            f.text("operation", "discover");
            if let Some(c) = r.cursor {
                f.u64("afterAcceptedAt", c.after_accepted_at);
                f.bytes("afterPlacementId", &c.after_placement_id.0);
            }
            f.0.push(("limit".into(), Value::Uint32(r.limit)));
        }
        Request::Recover(r) => {
            f.text("operation", "recover");
            f.bytes("placementId", &r.placement_id.0);
            f.bytes("messageId", &r.message_id.0);
            f.bytes("payloadHash", &r.payload_hash.0);
            f.bytes("recoveryId", &r.recovery_id.0);
            f.bytes("recipientPublicKey", &r.recipient_public_key);
            f.bytes("recipientSignature", &r.recipient_signature.0);
        }
        Request::Status(r) => {
            f.text("operation", "status");
            f.bytes("placementId", &r.placement_id.0);
        }
        Request::Renew(r) => {
            f.text("operation", "renew");
            f.bytes("placementId", &r.placement_id.0);
            f.u64("expectedExpiresAt", r.expected_expires_at);
            f.u64("additionalSeconds", r.additional_seconds);
        }
        Request::Remove(r) => {
            f.text("operation", "remove");
            f.bytes("placementId", &r.placement_id.0);
            f.bytes("removalId", &r.removal_id.0);
            f.u64("expectedExpiresAt", r.expected_expires_at);
            f.u64("minimumReleaseCredits", r.minimum_release_credits);
        }
    }
    f.finish()
}

/// Encode a response as the logical value a provider returns.
pub fn response_value(response: &Response) -> Value {
    tracing::trace!("Encoding a Messaging response");
    let mut f = Out::new();
    match response {
        Response::Store(r) => {
            f.text("operation", "store");
            f.bytes("placementId", &r.placement_id.0);
            f.bytes("messageId", &r.message_id.0);
            f.u64("acceptedAt", r.accepted_at);
            f.u64("expiresAt", r.expires_at);
            f.u64("price", r.price);
        }
        Response::Discover(r) => {
            f.text("operation", "discover");
            let entries = r.messages.iter().map(entry_value).collect();
            f.0.push(("messages".into(), Value::Array(entries)));
            f.0.push(("more".into(), Value::Bool(r.more)));
            f.u64("price", r.price);
        }
        Response::Recover(r) => {
            f.text("operation", "recover");
            let m = &r.message.metadata;
            f.bytes("placementId", &r.placement_id.0);
            f.bytes("messageId", &m.message_id.0);
            f.bytes("senderId", &m.sender_id.0);
            f.bytes("recipientId", &m.recipient_id.0);
            f.u64("senderCreatedAt", m.sender_created_at);
            f.text("payloadEncoding", m.encoding.name());
            f.u64("payloadSize", m.payload_size);
            f.bytes("payloadHash", &m.payload_hash.0);
            f.bytes("senderPublicKey", &r.message.sender_public_key);
            f.bytes("senderSignature", &r.message.sender_signature.0);
            f.bytes("payload", &r.message.payload);
            f.bytes("recoveryId", &r.receipt.recovery_id.0);
            f.u64("recoveredAt", r.receipt.recovered_at);
            f.bytes("recipientPublicKey", &r.receipt.recipient_public_key);
            f.bytes("recipientSignature", &r.receipt.recipient_signature.0);
            f.bytes("providerSignature", &r.receipt.provider_signature.0);
        }
        Response::Status(r) => {
            f.text("operation", "status");
            f.bytes("placementId", &r.placement_id.0);
            f.bytes("messageId", &r.message_id.0);
            f.bytes("senderId", &r.sender_id.0);
            f.bytes("recipientId", &r.recipient_id.0);
            f.bytes("payloadHash", &r.payload_hash.0);
            f.text("state", r.state.status_name().unwrap_or("pending"));
            f.u64("expiresAt", r.expires_at);
            if let Some(receipt) = &r.receipt {
                f.bytes("recoveryId", &receipt.recovery_id.0);
                f.u64("recoveredAt", receipt.recovered_at);
                f.bytes("recipientPublicKey", &receipt.recipient_public_key);
                f.bytes("recipientSignature", &receipt.recipient_signature.0);
                f.bytes("providerSignature", &receipt.provider_signature.0);
            }
            f.u64("price", r.price);
        }
        Response::Renew(r) => {
            f.text("operation", "renew");
            f.bytes("placementId", &r.placement_id.0);
            f.u64("previousExpiresAt", r.previous_expires_at);
            f.u64("expiresAt", r.expires_at);
            f.u64("price", r.price);
        }
        Response::Remove(r) => {
            f.text("operation", "remove");
            f.bytes("placementId", &r.placement_id.0);
            f.bytes("removalId", &r.removal_id.0);
            f.0.push(("removed".into(), Value::Bool(r.removed)));
            f.u64("releaseCredits", r.release_credits);
            f.u64("price", r.price);
        }
    }
    f.finish()
}

// One discovery entry.
fn entry_value(e: &Entry) -> Value {
    let mut f = Out::new();
    f.bytes("placementId", &e.placement_id.0);
    f.bytes("messageId", &e.message_id.0);
    f.bytes("senderId", &e.sender_id.0);
    f.u64("senderCreatedAt", e.sender_created_at);
    f.text("payloadEncoding", e.encoding.name());
    f.u64("payloadSize", e.payload_size);
    f.bytes("payloadHash", &e.payload_hash.0);
    f.u64("acceptedAt", e.accepted_at);
    f.u64("expiresAt", e.expires_at);
    f.finish()
}

/// Decode a response value for the requester, checking every required field's type and length.
/// # Errors
/// `Invalid` for a malformed value; `Unsupported` for an unknown operation or state.
pub fn parse_response(value: &Value) -> Result<Response, Error> {
    tracing::trace!("Decoding a Messaging response");
    let fields = Fields::new(value)?;
    Ok(match fields.text("operation")? {
        "store" => Response::Store(StoreOutput {
            placement_id: PlacementId(fields.array("placementId")?),
            message_id: MessageId(fields.array("messageId")?),
            accepted_at: fields.u64("acceptedAt")?,
            expires_at: fields.u64("expiresAt")?,
            price: fields.u64("price")?,
        }),
        "discover" => {
            let Some(Value::Array(items)) = fields.get("messages") else {
                return Err(Error::Invalid);
            };
            let messages = items.iter().map(parse_entry).collect::<Result<_, _>>()?;
            Response::Discover(DiscoverOutput {
                messages,
                more: fields.bool("more")?,
                price: fields.u64("price")?,
            })
        }
        "recover" => {
            let payload = fields.bytes("payload")?.to_vec();
            let payload_size = fields.u64("payloadSize")?;
            if payload_size == 0 || payload.len() as u64 != payload_size {
                return Err(Error::Invalid);
            }
            Response::Recover(RecoverOutput {
                placement_id: PlacementId(fields.array("placementId")?),
                message: SignedMessage {
                    metadata: Metadata {
                        message_id: MessageId(fields.array("messageId")?),
                        sender_id: IdentityId(fields.array("senderId")?),
                        recipient_id: IdentityId(fields.array("recipientId")?),
                        sender_created_at: fields.u64("senderCreatedAt")?,
                        encoding: Encoding::parse(fields.text("payloadEncoding")?)
                            .ok_or(Error::Unsupported)?,
                        payload_size,
                        payload_hash: PayloadHash(fields.array("payloadHash")?),
                    },
                    sender_public_key: fields.array("senderPublicKey")?,
                    sender_signature: Signature(fields.array("senderSignature")?),
                    payload,
                },
                receipt: parse_receipt(&fields)?,
            })
        }
        "status" => {
            let state = match fields.text("state")? {
                "pending" => State::Pending,
                "recovered" => State::Recovered,
                _ => return Err(Error::Unsupported),
            };
            // A recovered status carries the whole receipt and a pending one none of it.
            let receipt = match state {
                State::Recovered => Some(parse_receipt(&fields)?),
                _ if fields.has("recoveryId") => return Err(Error::Invalid),
                _ => None,
            };
            Response::Status(StatusOutput {
                placement_id: PlacementId(fields.array("placementId")?),
                message_id: MessageId(fields.array("messageId")?),
                sender_id: IdentityId(fields.array("senderId")?),
                recipient_id: IdentityId(fields.array("recipientId")?),
                payload_hash: PayloadHash(fields.array("payloadHash")?),
                state,
                expires_at: fields.u64("expiresAt")?,
                receipt,
                price: fields.u64("price")?,
            })
        }
        "renew" => Response::Renew(RenewOutput {
            placement_id: PlacementId(fields.array("placementId")?),
            previous_expires_at: fields.u64("previousExpiresAt")?,
            expires_at: fields.u64("expiresAt")?,
            price: fields.u64("price")?,
        }),
        "remove" => Response::Remove(RemoveOutput {
            placement_id: PlacementId(fields.array("placementId")?),
            removal_id: RemovalId(fields.array("removalId")?),
            removed: fields.bool("removed")?,
            release_credits: fields.u64("releaseCredits")?,
            price: fields.u64("price")?,
        }),
        _ => return Err(Error::Unsupported),
    })
}

// The receipt fields shared by a recovery and a recovered status.
fn parse_receipt(fields: &Fields) -> Result<Receipt, Error> {
    Ok(Receipt {
        recovery_id: RecoveryId(fields.array("recoveryId")?),
        recovered_at: fields.u64("recoveredAt")?,
        recipient_public_key: fields.array("recipientPublicKey")?,
        recipient_signature: Signature(fields.array("recipientSignature")?),
        provider_signature: Signature(fields.array("providerSignature")?),
    })
}

// One discovery entry.
fn parse_entry(value: &Value) -> Result<Entry, Error> {
    let f = Fields::new(value)?;
    Ok(Entry {
        placement_id: PlacementId(f.array("placementId")?),
        message_id: MessageId(f.array("messageId")?),
        sender_id: IdentityId(f.array("senderId")?),
        sender_created_at: f.u64("senderCreatedAt")?,
        encoding: Encoding::parse(f.text("payloadEncoding")?).ok_or(Error::Unsupported)?,
        payload_size: f.u64("payloadSize")?,
        payload_hash: PayloadHash(f.array("payloadHash")?),
        accepted_at: f.u64("acceptedAt")?,
        expires_at: f.u64("expiresAt")?,
    })
}

/// Decode a check challenge.
/// # Errors
/// `Invalid` for a malformed value.
pub fn parse_check_input(value: &Value) -> Result<CheckInput, Error> {
    tracing::trace!("Decoding a Messaging check challenge");
    let f = Fields::new(value)?;
    Ok(CheckInput {
        placement_id: PlacementId(f.array("placementId")?),
        offset: f.u64("offset")?,
    })
}

/// Encode a check challenge.
pub fn check_input_value(input: &CheckInput) -> Value {
    tracing::trace!("Encoding a Messaging check challenge");
    let mut f = Out::new();
    f.bytes("placementId", &input.placement_id.0);
    f.u64("offset", input.offset);
    f.finish()
}

/// Encode check evidence.
pub fn check_output_value(output: &CheckOutput) -> Value {
    tracing::trace!("Encoding Messaging check evidence");
    let mut f = Out::new();
    f.bytes("placementId", &output.placement_id.0);
    f.u64("offset", output.offset);
    f.bytes("value", &[output.value]);
    f.finish()
}

/// Decode check evidence: exactly one byte.
/// # Errors
/// `Invalid` for a malformed value or a value that is not one byte.
pub fn parse_check_output(value: &Value) -> Result<CheckOutput, Error> {
    tracing::trace!("Decoding Messaging check evidence");
    let f = Fields::new(value)?;
    let [byte] = f.bytes("value")? else {
        return Err(Error::Invalid);
    };
    Ok(CheckOutput {
        placement_id: PlacementId(f.array("placementId")?),
        offset: f.u64("offset")?,
        value: *byte,
    })
}

// Typed read access to one object's fields, rejecting duplicates up front.
struct Fields<'a>(&'a [(String, Value)]);
impl<'a> Fields<'a> {
    // The fields of an object value with unique names.
    fn new(value: &'a Value) -> Result<Self, Error> {
        let Value::Object(fields) = value else {
            return Err(Error::Invalid);
        };
        for (index, (name, _)) in fields.iter().enumerate() {
            if fields[..index].iter().any(|(other, _)| other == name) {
                return Err(Error::Invalid);
            }
        }
        Ok(Self(fields))
    }
    fn get(&self, name: &str) -> Option<&'a Value> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
    fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
    fn text(&self, name: &str) -> Result<&'a str, Error> {
        match self.get(name) {
            Some(Value::String(s)) => Ok(s),
            _ => Err(Error::Invalid),
        }
    }
    fn bytes(&self, name: &str) -> Result<&'a [u8], Error> {
        match self.get(name) {
            Some(Value::Bytes(b)) => Ok(b),
            _ => Err(Error::Invalid),
        }
    }
    // Exactly N bytes.
    fn array<const N: usize>(&self, name: &str) -> Result<[u8; N], Error> {
        self.bytes(name)?.try_into().map_err(|_| Error::Invalid)
    }
    fn u64(&self, name: &str) -> Result<u64, Error> {
        match self.get(name) {
            Some(Value::Uint64(n)) => Ok(*n),
            _ => Err(Error::Invalid),
        }
    }
    fn u32(&self, name: &str) -> Result<u32, Error> {
        match self.get(name) {
            Some(Value::Uint32(n)) => Ok(*n),
            _ => Err(Error::Invalid),
        }
    }
    fn bool(&self, name: &str) -> Result<bool, Error> {
        match self.get(name) {
            Some(Value::Bool(b)) => Ok(*b),
            _ => Err(Error::Invalid),
        }
    }
}

// Builder of an object value in field order.
struct Out(Vec<(String, Value)>);
impl Out {
    fn new() -> Self {
        Self(Vec::new())
    }
    fn text(&mut self, name: &str, value: &str) {
        self.0.push((name.into(), Value::String(value.into())));
    }
    fn bytes(&mut self, name: &str, value: &[u8]) {
        self.0.push((name.into(), Value::Bytes(value.to_vec())));
    }
    fn u64(&mut self, name: &str, value: u64) {
        self.0.push((name.into(), Value::Uint64(value)));
    }
    fn finish(self) -> Value {
        Value::Object(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message() -> SignedMessage {
        SignedMessage {
            metadata: Metadata {
                message_id: MessageId([1; 32]),
                sender_id: IdentityId([2; 32]),
                recipient_id: IdentityId([3; 32]),
                sender_created_at: 1000,
                encoding: Encoding::Opaque,
                payload_size: 3,
                payload_hash: PayloadHash([4; 32]),
            },
            sender_public_key: [5; 32],
            sender_signature: Signature([6; 64]),
            payload: b"abc".to_vec(),
        }
    }
    fn receipt() -> Receipt {
        Receipt {
            recovery_id: RecoveryId([7; 32]),
            recovered_at: 1005,
            recipient_public_key: [8; 32],
            recipient_signature: Signature([9; 64]),
            provider_signature: Signature([10; 64]),
        }
    }
    fn requests() -> Vec<Request> {
        vec![
            Request::Store(StoreInput {
                message: message(),
                retention_seconds: 10,
            }),
            Request::Discover(DiscoverInput {
                cursor: None,
                limit: 2,
            }),
            Request::Discover(DiscoverInput {
                cursor: Some(Cursor {
                    after_accepted_at: 1000,
                    after_placement_id: PlacementId([11; 32]),
                }),
                limit: 5,
            }),
            Request::Recover(RecoverInput {
                placement_id: PlacementId([12; 32]),
                message_id: MessageId([1; 32]),
                payload_hash: PayloadHash([4; 32]),
                recovery_id: RecoveryId([7; 32]),
                recipient_public_key: [8; 32],
                recipient_signature: Signature([9; 64]),
            }),
            Request::Status(StatusInput {
                placement_id: PlacementId([12; 32]),
            }),
            Request::Renew(RenewInput {
                placement_id: PlacementId([12; 32]),
                expected_expires_at: 1010,
                additional_seconds: 4,
            }),
            Request::Remove(RemoveInput {
                placement_id: PlacementId([12; 32]),
                removal_id: RemovalId([13; 32]),
                expected_expires_at: 1010,
                minimum_release_credits: 4,
            }),
        ]
    }
    fn responses() -> Vec<Response> {
        let entry = Entry {
            placement_id: PlacementId([12; 32]),
            message_id: MessageId([1; 32]),
            sender_id: IdentityId([2; 32]),
            sender_created_at: 1000,
            encoding: Encoding::IdentityEncryptedV1,
            payload_size: 3,
            payload_hash: PayloadHash([4; 32]),
            accepted_at: 1000,
            expires_at: 1010,
        };
        let status = |state, receipt| StatusOutput {
            placement_id: PlacementId([12; 32]),
            message_id: MessageId([1; 32]),
            sender_id: IdentityId([2; 32]),
            recipient_id: IdentityId([3; 32]),
            payload_hash: PayloadHash([4; 32]),
            state,
            expires_at: 1010,
            receipt,
            price: 1,
        };
        vec![
            Response::Store(StoreOutput {
                placement_id: PlacementId([12; 32]),
                message_id: MessageId([1; 32]),
                accepted_at: 1000,
                expires_at: 1010,
                price: 17,
            }),
            Response::Discover(DiscoverOutput {
                messages: vec![entry.clone(), entry],
                more: true,
                price: 1,
            }),
            Response::Discover(DiscoverOutput {
                messages: vec![],
                more: false,
                price: 1,
            }),
            Response::Recover(RecoverOutput {
                placement_id: PlacementId([12; 32]),
                message: message(),
                receipt: receipt(),
            }),
            Response::Status(status(State::Pending, None)),
            Response::Status(status(State::Recovered, Some(receipt()))),
            Response::Renew(RenewOutput {
                placement_id: PlacementId([12; 32]),
                previous_expires_at: 1010,
                expires_at: 1014,
                price: 6,
            }),
            Response::Remove(RemoveOutput {
                placement_id: PlacementId([12; 32]),
                removal_id: RemovalId([13; 32]),
                removed: true,
                release_credits: 5,
                price: 2,
            }),
        ]
    }
    fn fields_of(value: &Value) -> Vec<(String, Value)> {
        let Value::Object(f) = value else {
            panic!("object expected")
        };
        f.clone()
    }

    // Every request and response survives an encode and decode round trip exactly, so both sides agree on the logical values.
    #[test]
    fn requests_and_responses_round_trip() {
        for request in requests() {
            assert_eq!(parse_request(&request_value(&request)), Ok(request));
        }
        for response in responses() {
            assert_eq!(parse_response(&response_value(&response)), Ok(response));
        }
        let challenge = CheckInput {
            placement_id: PlacementId([12; 32]),
            offset: 1,
        };
        assert_eq!(
            parse_check_input(&check_input_value(&challenge)),
            Ok(challenge)
        );
        let evidence = CheckOutput {
            placement_id: PlacementId([12; 32]),
            offset: 1,
            value: b'b',
        };
        assert_eq!(
            parse_check_output(&check_output_value(&evidence)),
            Ok(evidence)
        );
    }

    // Removing any field a request needs fails, except the two optional cursor fields which must come as a pair.
    #[test]
    fn every_required_request_field_is_required() {
        for request in requests() {
            let value = request_value(&request);
            let names: Vec<String> = fields_of(&value).into_iter().map(|(n, _)| n).collect();
            for name in names.iter().filter(|n| *n != "operation") {
                let mut fields = fields_of(&value);
                fields.retain(|(n, _)| n != name);
                assert_eq!(
                    parse_request(&Value::Object(fields)),
                    Err(Error::Invalid),
                    "{name}"
                );
            }
        }
    }

    // A field with the wrong logical type (bytes for an integer, an integer for bytes, text for either) is invalid.
    #[test]
    fn wrong_types_are_invalid() {
        for request in requests() {
            let value = request_value(&request);
            for (name, original) in fields_of(&value) {
                if name == "operation" || name == "payloadEncoding" {
                    continue;
                }
                let wrong = match original {
                    Value::Uint64(_) | Value::Uint32(_) => Value::Bytes(vec![0; 8]),
                    Value::Bytes(_) => Value::Uint64(1),
                    _ => Value::String("x".into()),
                };
                let mut fields = fields_of(&value);
                for entry in &mut fields {
                    if entry.0 == name {
                        entry.1 = wrong.clone();
                    }
                }
                assert_eq!(
                    parse_request(&Value::Object(fields)),
                    Err(Error::Invalid),
                    "{name}"
                );
            }
        }
        // A limit sent as a 64-bit integer is not the specified uint32.
        let mut fields = fields_of(&request_value(&Request::Discover(DiscoverInput {
            cursor: None,
            limit: 2,
        })));
        fields[1].1 = Value::Uint64(2);
        assert_eq!(parse_request(&Value::Object(fields)), Err(Error::Invalid));
    }

    // Fixed-length fields accept exactly their length: one byte fewer or more fails for every identifier, key and signature.
    #[test]
    fn fixed_lengths_are_exact() {
        for request in requests() {
            let value = request_value(&request);
            for (name, original) in fields_of(&value) {
                let Value::Bytes(bytes) = original else {
                    continue;
                };
                if name == "payload" {
                    continue;
                }
                for length in [bytes.len() - 1, bytes.len() + 1, 0] {
                    let mut fields = fields_of(&value);
                    for entry in &mut fields {
                        if entry.0 == name {
                            entry.1 = Value::Bytes(vec![1; length]);
                        }
                    }
                    assert_eq!(
                        parse_request(&Value::Object(fields)),
                        Err(Error::Invalid),
                        "{name} {length}"
                    );
                }
            }
        }
    }

    // A field that belongs only to another operation is refused; an unknown field is inert; duplicates are refused.
    #[test]
    fn foreign_unknown_and_duplicate_fields() {
        let status = request_value(&Request::Status(StatusInput {
            placement_id: PlacementId([12; 32]),
        }));
        for (name, extra) in [
            ("payload", Value::Bytes(vec![1])),
            ("limit", Value::Uint32(1)),
            ("removalId", Value::Bytes(vec![0; 32])),
            ("retentionSeconds", Value::Uint64(1)),
        ] {
            let mut fields = fields_of(&status);
            fields.push((name.into(), extra));
            assert_eq!(
                parse_request(&Value::Object(fields)),
                Err(Error::Invalid),
                "{name}"
            );
        }
        let mut fields = fields_of(&status);
        fields.push(("futureExtension".into(), Value::Bool(true)));
        assert!(parse_request(&Value::Object(fields)).is_ok());
        let mut fields = fields_of(&status);
        fields.push(("placementId".into(), Value::Bytes(vec![12; 32])));
        assert_eq!(parse_request(&Value::Object(fields)), Err(Error::Invalid));
        assert_eq!(parse_request(&Value::Bool(true)), Err(Error::Invalid));
    }

    // Unknown operations and payload encodings are unsupported; a missing or non-text operation is invalid.
    #[test]
    fn unknown_operations_and_encodings() {
        let mut fields = fields_of(&request_value(&requests()[0]));
        fields[0].1 = Value::String("archive".into());
        assert_eq!(
            parse_request(&Value::Object(fields.clone())),
            Err(Error::Unsupported)
        );
        fields[0].1 = Value::String("store".into());
        for entry in &mut fields {
            if entry.0 == "payloadEncoding" {
                entry.1 = Value::String("rot13".into());
            }
        }
        assert_eq!(
            parse_request(&Value::Object(fields.clone())),
            Err(Error::Unsupported)
        );
        fields.retain(|(n, _)| n != "operation");
        assert_eq!(parse_request(&Value::Object(fields)), Err(Error::Invalid));
    }

    // The payload must be nonempty and exactly as long as payloadSize says, for requests and recovered responses.
    #[test]
    fn payload_size_must_match() {
        for (size, payload) in [
            (3u64, b"ab".to_vec()),
            (3, b"abcd".to_vec()),
            (0, vec![]),
            (1, vec![]),
        ] {
            let mut fields = fields_of(&request_value(&requests()[0]));
            for entry in &mut fields {
                match entry.0.as_str() {
                    "payloadSize" => entry.1 = Value::Uint64(size),
                    "payload" => entry.1 = Value::Bytes(payload.clone()),
                    _ => {}
                }
            }
            assert_eq!(
                parse_request(&Value::Object(fields)),
                Err(Error::Invalid),
                "{size}"
            );
        }
        let mut fields = fields_of(&response_value(&responses()[3]));
        for entry in &mut fields {
            if entry.0 == "payload" {
                entry.1 = Value::Bytes(b"ab".to_vec());
            }
        }
        assert_eq!(parse_response(&Value::Object(fields)), Err(Error::Invalid));
    }

    // Discovery needs a positive limit and both cursor fields or neither.
    #[test]
    fn discovery_cursor_and_limit_rules() {
        let page = |edit: fn(&mut Vec<(String, Value)>)| {
            let mut fields = fields_of(&request_value(&requests()[2]));
            edit(&mut fields);
            parse_request(&Value::Object(fields))
        };
        assert!(page(|_| {}).is_ok());
        assert_eq!(
            page(|f| f.retain(|(n, _)| n != "afterPlacementId")),
            Err(Error::Invalid)
        );
        assert_eq!(
            page(|f| f.retain(|(n, _)| n != "afterAcceptedAt")),
            Err(Error::Invalid)
        );
        assert!(page(|f| f.retain(|(n, _)| !n.starts_with("after"))).is_ok());
        assert_eq!(
            page(|f| f.iter_mut().for_each(|e| if e.0 == "limit" {
                e.1 = Value::Uint32(0)
            })),
            Err(Error::Invalid)
        );
    }

    // A pending status carries no receipt and a recovered one carries all of it; a state other than the two is unsupported;
    // check evidence is exactly one byte.
    #[test]
    fn response_consistency_rules() {
        let pending = fields_of(&response_value(&responses()[4]));
        let recovered = fields_of(&response_value(&responses()[5]));
        let mut with_receipt = pending.clone();
        with_receipt.push(("recoveryId".into(), Value::Bytes(vec![7; 32])));
        assert_eq!(
            parse_response(&Value::Object(with_receipt)),
            Err(Error::Invalid)
        );
        let mut without = recovered.clone();
        without.retain(|(n, _)| n != "providerSignature");
        assert_eq!(parse_response(&Value::Object(without)), Err(Error::Invalid));
        let mut odd = pending.clone();
        for entry in &mut odd {
            if entry.0 == "state" {
                entry.1 = Value::String("removed".into());
            }
        }
        assert_eq!(parse_response(&Value::Object(odd)), Err(Error::Unsupported));
        let mut evidence = fields_of(&check_output_value(&CheckOutput {
            placement_id: PlacementId([1; 32]),
            offset: 0,
            value: 1,
        }));
        for entry in &mut evidence {
            if entry.0 == "value" {
                entry.1 = Value::Bytes(vec![1, 2]);
            }
        }
        assert_eq!(
            parse_check_output(&Value::Object(evidence)),
            Err(Error::Invalid)
        );
    }
}
