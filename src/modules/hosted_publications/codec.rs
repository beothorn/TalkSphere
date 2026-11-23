//! Decoding and encoding of hosted publication operations between typed values and the Services logical value model.
//!
//! Callers: the provider (decode requests, encode responses) and the requester (encode requests, decode responses). Every field has its
//! exact logical type and fixed length; a field that belongs only to another operation is rejected, an unknown field follows the
//! compatible-extension rule (ignored), duplicate names and wrong types are invalid, and a content length must equal the declared size.
//! Wire encoding of these values belongs to Peer Protocol.
use super::{
    error::Error,
    ids::{ContentHash, PlacementId, PublicationId, RemovalId, Signature},
    metadata::{Kind, Metadata, Publication, Visibility},
    operation::{
        CheckOutput, CheckRequest, GetOutput, GetRequest, ListEntry, ListOutput, ListRequest,
        RemoveOutput, RemoveRequest, RenewOutput, RenewRequest, StoreOutput, StoreRequest,
    },
};
use crate::modules::{identity::api::IdentityId, services::value::Value};

/// A decoded request of one of the five operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Retain a publication.
    Store(StoreRequest),
    /// List a publisher's publications.
    List(ListRequest),
    /// Return one publication.
    Get(GetRequest),
    /// Extend retention.
    Renew(RenewRequest),
    /// End a placement early.
    Remove(RemoveRequest),
}

/// A decoded response of one of the five operations, each with the credits charged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// Result of a store and its price.
    Store(StoreOutput, u64),
    /// One page of listing and its price.
    List(ListOutput, u64),
    /// A returned publication and its price.
    Get(GetOutput, u64),
    /// Result of a renewal and its price.
    Renew(RenewOutput, u64),
    /// Result of a removal; its price is inside.
    Remove(RemoveOutput),
}

// Every field name any operation input uses, so a field of another operation can be recognized and refused.
const KNOWN_INPUT: [&str; 22] = [
    "placementId",
    "publicationId",
    "publisherId",
    "kind",
    "sequence",
    "publishedAt",
    "visibility",
    "audience",
    "contentSize",
    "contentHash",
    "publisherPublicKey",
    "publisherSignature",
    "content",
    "retentionSeconds",
    "beforeSequence",
    "beforePublicationId",
    "limit",
    "maximumBytes",
    "expectedExpiresAt",
    "additionalSeconds",
    "removalId",
    "minimumReleaseCredits",
];
const STORE_FIELDS: [&str; 13] = [
    "publicationId",
    "publisherId",
    "kind",
    "sequence",
    "publishedAt",
    "visibility",
    "audience",
    "contentSize",
    "contentHash",
    "publisherPublicKey",
    "publisherSignature",
    "retentionSeconds",
    "content",
];
const LIST_FIELDS: [&str; 5] = [
    "publisherId",
    "kind",
    "beforeSequence",
    "beforePublicationId",
    "limit",
];
const GET_FIELDS: [&str; 3] = ["publisherId", "publicationId", "maximumBytes"];
const RENEW_FIELDS: [&str; 3] = ["placementId", "expectedExpiresAt", "additionalSeconds"];
const REMOVE_FIELDS: [&str; 4] = [
    "placementId",
    "removalId",
    "expectedExpiresAt",
    "minimumReleaseCredits",
];

/// Decode a request value.
/// # Errors
/// `Invalid` for a non-object, duplicate names, a missing or wrongly typed or sized field, a field of another operation, a single
/// cursor field or a zero limit, maximum or content; `Unsupported` for an unknown operation, kind or visibility.
pub fn parse_request(value: &Value) -> Result<Request, Error> {
    tracing::trace!("Decoding a hosted publication request");
    let fields = Fields::new(value)?;
    let allowed: &[&str] = match fields.text("operation")? {
        "store" => &STORE_FIELDS,
        "list" => &LIST_FIELDS,
        "get" => &GET_FIELDS,
        "renew" => &RENEW_FIELDS,
        "remove" => &REMOVE_FIELDS,
        _ => return Err(Error::Unsupported),
    };
    for (name, _) in fields.0 {
        if KNOWN_INPUT.contains(&name.as_str()) && !allowed.contains(&name.as_str()) {
            return Err(Error::Invalid);
        }
    }
    Ok(match fields.text("operation")? {
        "store" => Request::Store(parse_store(&fields)?),
        "list" => Request::List(parse_list(&fields)?),
        "get" => {
            let maximum_bytes = fields.u64("maximumBytes")?;
            if maximum_bytes == 0 {
                return Err(Error::Invalid);
            }
            Request::Get(GetRequest {
                publisher_id: IdentityId(fields.array("publisherId")?),
                publication_id: PublicationId(fields.array("publicationId")?),
                maximum_bytes,
            })
        }
        "renew" => Request::Renew(RenewRequest {
            placement_id: PlacementId(fields.array("placementId")?),
            expected_expires_at: fields.u64("expectedExpiresAt")?,
            additional_seconds: fields.u64("additionalSeconds")?,
        }),
        _ => Request::Remove(RemoveRequest {
            placement_id: PlacementId(fields.array("placementId")?),
            removal_id: RemovalId(fields.array("removalId")?),
            expected_expires_at: fields.u64("expectedExpiresAt")?,
            minimum_release_credits: fields.u64("minimumReleaseCredits")?,
        }),
    })
}

// A store request: a complete signed publication, retention and a content length exactly matching the declared size.
fn parse_store(fields: &Fields) -> Result<StoreRequest, Error> {
    Ok(StoreRequest {
        publication: parse_publication(fields)?,
        retention_seconds: fields.u64("retentionSeconds")?,
    })
}

// The publication fields shared by a store request and a get response.
fn parse_publication(fields: &Fields) -> Result<Publication, Error> {
    let content = fields.bytes("content")?.to_vec();
    let content_size = fields.u64("contentSize")?;
    if content_size == 0 || content.len() as u64 != content_size {
        return Err(Error::Invalid);
    }
    let audience = match fields.get("audience") {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::Bytes(bytes) => bytes
                    .as_slice()
                    .try_into()
                    .map(IdentityId)
                    .map_err(|_| Error::Invalid),
                _ => Err(Error::Invalid),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(Error::Invalid),
    };
    Ok(Publication {
        metadata: Metadata {
            publication_id: PublicationId(fields.array("publicationId")?),
            publisher_id: IdentityId(fields.array("publisherId")?),
            kind: Kind::from_name(fields.text("kind")?)?,
            sequence: fields.u64("sequence")?,
            published_at: fields.u64("publishedAt")?,
            visibility: Visibility::from_name(fields.text("visibility")?)?,
            audience,
            content_size,
            content_hash: ContentHash(fields.array("contentHash")?),
        },
        publisher_public_key: fields.array("publisherPublicKey")?,
        publisher_signature: Signature(fields.array("publisherSignature")?),
        content,
    })
}

// A list request: a positive limit and either both cursor fields or neither.
fn parse_list(fields: &Fields) -> Result<ListRequest, Error> {
    let limit = fields.u32("limit")?;
    if limit == 0 {
        return Err(Error::Invalid);
    }
    let before = match (
        fields.has("beforeSequence"),
        fields.has("beforePublicationId"),
    ) {
        (false, false) => None,
        (true, true) => Some((
            fields.u64("beforeSequence")?,
            PublicationId(fields.array("beforePublicationId")?),
        )),
        _ => return Err(Error::Invalid),
    };
    Ok(ListRequest {
        publisher_id: IdentityId(fields.array("publisherId")?),
        kind: Kind::from_name(fields.text("kind")?)?,
        before,
        limit,
    })
}

/// Encode a request as the logical value a requester sends.
pub fn request_value(request: &Request) -> Value {
    tracing::trace!("Encoding a hosted publication request");
    let mut f = Out::new();
    match request {
        Request::Store(r) => {
            f.text("operation", "store");
            publication_fields(&mut f, &r.publication);
            f.u64("retentionSeconds", r.retention_seconds);
        }
        Request::List(r) => {
            f.text("operation", "list");
            f.bytes("publisherId", &r.publisher_id.0);
            f.text("kind", r.kind.name());
            if let Some((sequence, publication_id)) = r.before {
                f.u64("beforeSequence", sequence);
                f.bytes("beforePublicationId", &publication_id.0);
            }
            f.0.push(("limit".into(), Value::Uint32(r.limit)));
        }
        Request::Get(r) => {
            f.text("operation", "get");
            f.bytes("publisherId", &r.publisher_id.0);
            f.bytes("publicationId", &r.publication_id.0);
            f.u64("maximumBytes", r.maximum_bytes);
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

// The publication fields shared by a store request and a get response, in specification order.
fn publication_fields(f: &mut Out, publication: &Publication) {
    let m = &publication.metadata;
    f.bytes("publicationId", &m.publication_id.0);
    f.bytes("publisherId", &m.publisher_id.0);
    f.text("kind", m.kind.name());
    f.u64("sequence", m.sequence);
    f.u64("publishedAt", m.published_at);
    f.text("visibility", m.visibility.name());
    let audience = m
        .audience
        .iter()
        .map(|id| Value::Bytes(id.0.to_vec()))
        .collect();
    f.0.push(("audience".into(), Value::Array(audience)));
    f.u64("contentSize", m.content_size);
    f.bytes("contentHash", &m.content_hash.0);
    f.bytes("publisherPublicKey", &publication.publisher_public_key);
    f.bytes("publisherSignature", &publication.publisher_signature.0);
    f.bytes("content", &publication.content);
}

/// Encode a response as the logical value a provider returns.
pub fn response_value(response: &Response) -> Value {
    tracing::trace!("Encoding a hosted publication response");
    let mut f = Out::new();
    match response {
        Response::Store(r, price) => {
            f.text("operation", "store");
            f.bytes("placementId", &r.placement_id.0);
            f.bytes("publicationId", &r.publication_id.0);
            f.u64("acceptedAt", r.accepted_at);
            f.u64("expiresAt", r.expires_at);
            f.u64("price", *price);
        }
        Response::List(r, price) => {
            f.text("operation", "list");
            f.bytes("publisherId", &r.publisher_id.0);
            let entries = r.entries.iter().map(entry_value).collect();
            f.0.push(("entries".into(), Value::Array(entries)));
            f.0.push(("more".into(), Value::Bool(r.more)));
            f.u64("price", *price);
        }
        Response::Get(r, price) => {
            f.text("operation", "get");
            publication_fields(&mut f, &r.publication);
            f.u64("price", *price);
        }
        Response::Renew(r, price) => {
            f.text("operation", "renew");
            f.bytes("placementId", &r.placement_id.0);
            f.u64("previousExpiresAt", r.previous_expires_at);
            f.u64("expiresAt", r.expires_at);
            f.u64("price", *price);
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

// One listed entry.
fn entry_value(e: &ListEntry) -> Value {
    let mut f = Out::new();
    f.bytes("placementId", &e.placement_id.0);
    f.bytes("publicationId", &e.publication_id.0);
    f.text("kind", e.kind.name());
    f.u64("sequence", e.sequence);
    f.u64("publishedAt", e.published_at);
    f.text("visibility", e.visibility.name());
    f.u64("contentSize", e.content_size);
    f.bytes("contentHash", &e.content_hash.0);
    f.u64("acceptedAt", e.accepted_at);
    f.u64("expiresAt", e.expires_at);
    f.finish()
}

/// Decode a response value for the requester, checking every required field's type and length.
/// # Errors
/// `Invalid` for a malformed value; `Unsupported` for an unknown operation, kind or visibility.
pub fn parse_response(value: &Value) -> Result<Response, Error> {
    tracing::trace!("Decoding a hosted publication response");
    let fields = Fields::new(value)?;
    Ok(match fields.text("operation")? {
        "store" => Response::Store(
            StoreOutput {
                placement_id: PlacementId(fields.array("placementId")?),
                publication_id: PublicationId(fields.array("publicationId")?),
                accepted_at: fields.u64("acceptedAt")?,
                expires_at: fields.u64("expiresAt")?,
            },
            fields.u64("price")?,
        ),
        "list" => {
            let Some(Value::Array(items)) = fields.get("entries") else {
                return Err(Error::Invalid);
            };
            let entries = items.iter().map(parse_entry).collect::<Result<_, _>>()?;
            Response::List(
                ListOutput {
                    publisher_id: IdentityId(fields.array("publisherId")?),
                    entries,
                    more: fields.bool("more")?,
                },
                fields.u64("price")?,
            )
        }
        "get" => Response::Get(
            GetOutput {
                publication: parse_publication(&fields)?,
            },
            fields.u64("price")?,
        ),
        "renew" => Response::Renew(
            RenewOutput {
                placement_id: PlacementId(fields.array("placementId")?),
                previous_expires_at: fields.u64("previousExpiresAt")?,
                expires_at: fields.u64("expiresAt")?,
            },
            fields.u64("price")?,
        ),
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

// One listed entry.
fn parse_entry(value: &Value) -> Result<ListEntry, Error> {
    let f = Fields::new(value)?;
    Ok(ListEntry {
        placement_id: PlacementId(f.array("placementId")?),
        publication_id: PublicationId(f.array("publicationId")?),
        kind: Kind::from_name(f.text("kind")?)?,
        sequence: f.u64("sequence")?,
        published_at: f.u64("publishedAt")?,
        visibility: Visibility::from_name(f.text("visibility")?)?,
        content_size: f.u64("contentSize")?,
        content_hash: ContentHash(f.array("contentHash")?),
        accepted_at: f.u64("acceptedAt")?,
        expires_at: f.u64("expiresAt")?,
    })
}

/// Decode a check challenge.
/// # Errors
/// `Invalid` for a malformed value.
pub fn parse_check_input(value: &Value) -> Result<CheckRequest, Error> {
    tracing::trace!("Decoding a hosted check challenge");
    let f = Fields::new(value)?;
    Ok(CheckRequest {
        placement_id: PlacementId(f.array("placementId")?),
        offset: f.u64("offset")?,
    })
}

/// Encode a check challenge.
pub fn check_input_value(input: &CheckRequest) -> Value {
    tracing::trace!("Encoding a hosted check challenge");
    let mut f = Out::new();
    f.bytes("placementId", &input.placement_id.0);
    f.u64("offset", input.offset);
    f.finish()
}

/// Encode check evidence.
pub fn check_output_value(output: &CheckOutput) -> Value {
    tracing::trace!("Encoding hosted check evidence");
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
    tracing::trace!("Decoding hosted check evidence");
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

    fn publication() -> Publication {
        Publication {
            metadata: Metadata {
                publication_id: PublicationId([1; 32]),
                publisher_id: IdentityId([2; 32]),
                kind: Kind::FeedPost,
                sequence: 7,
                published_at: 1002,
                visibility: Visibility::Restricted,
                audience: vec![IdentityId([3; 32]), IdentityId([4; 32])],
                content_size: 3,
                content_hash: ContentHash([5; 32]),
            },
            publisher_public_key: [6; 32],
            publisher_signature: Signature([7; 64]),
            content: b"abc".to_vec(),
        }
    }

    fn requests() -> Vec<Request> {
        vec![
            Request::Store(StoreRequest {
                publication: publication(),
                retention_seconds: 10,
            }),
            Request::List(ListRequest {
                publisher_id: IdentityId([2; 32]),
                kind: Kind::Page,
                before: Some((7, PublicationId([1; 32]))),
                limit: 2,
            }),
            Request::List(ListRequest {
                publisher_id: IdentityId([2; 32]),
                kind: Kind::FeedPost,
                before: None,
                limit: 1,
            }),
            Request::Get(GetRequest {
                publisher_id: IdentityId([2; 32]),
                publication_id: PublicationId([1; 32]),
                maximum_bytes: 100,
            }),
            Request::Renew(RenewRequest {
                placement_id: PlacementId([8; 32]),
                expected_expires_at: 1010,
                additional_seconds: 5,
            }),
            Request::Remove(RemoveRequest {
                placement_id: PlacementId([8; 32]),
                removal_id: RemovalId([9; 32]),
                expected_expires_at: 1010,
                minimum_release_credits: 4,
            }),
        ]
    }

    // Every request survives encode then decode unchanged, including an empty and a two-reader audience and both cursor shapes.
    #[test]
    fn requests_round_trip() {
        for request in requests() {
            assert_eq!(parse_request(&request_value(&request)), Ok(request));
        }
        let mut public = publication();
        public.metadata.visibility = Visibility::Public;
        public.metadata.audience.clear();
        let request = Request::Store(StoreRequest {
            publication: public,
            retention_seconds: 1,
        });
        assert_eq!(parse_request(&request_value(&request)), Ok(request));
    }

    // Every response survives encode then decode unchanged, with the price in each.
    #[test]
    fn responses_round_trip() {
        let entry = ListEntry {
            placement_id: PlacementId([1; 32]),
            publication_id: PublicationId([2; 32]),
            kind: Kind::Page,
            sequence: 3,
            published_at: 4,
            visibility: Visibility::Public,
            content_size: 5,
            content_hash: ContentHash([6; 32]),
            accepted_at: 7,
            expires_at: 8,
        };
        for response in [
            Response::Store(
                StoreOutput {
                    placement_id: PlacementId([1; 32]),
                    publication_id: PublicationId([2; 32]),
                    accepted_at: 1000,
                    expires_at: 1010,
                },
                17,
            ),
            Response::List(
                ListOutput {
                    publisher_id: IdentityId([2; 32]),
                    entries: vec![entry, entry],
                    more: true,
                },
                1,
            ),
            Response::List(
                ListOutput {
                    publisher_id: IdentityId([2; 32]),
                    entries: vec![],
                    more: false,
                },
                1,
            ),
            Response::Get(
                GetOutput {
                    publication: publication(),
                },
                2,
            ),
            Response::Renew(
                RenewOutput {
                    placement_id: PlacementId([1; 32]),
                    previous_expires_at: 1010,
                    expires_at: 1020,
                },
                15,
            ),
            Response::Remove(RemoveOutput {
                placement_id: PlacementId([1; 32]),
                removal_id: RemovalId([2; 32]),
                removed: true,
                release_credits: 5,
                price: 2,
            }),
        ] {
            assert_eq!(parse_response(&response_value(&response)), Ok(response));
        }
    }

    // Replace or add one field of an encoded request.
    fn with(request: &Request, name: &str, value: Option<Value>) -> Value {
        let Value::Object(mut fields) = request_value(request) else {
            unreachable!()
        };
        fields.retain(|(n, _)| n != name);
        if let Some(value) = value {
            fields.push((name.into(), value));
        }
        Value::Object(fields)
    }

    // HP-03/HP-05: every required field must be present with its exact type and length, a content length must equal the declared size,
    // one cursor field alone is invalid, zero limits and maximums are invalid, and nothing is accepted with duplicate names.
    #[test]
    fn malformed_requests_are_rejected() {
        let all = requests();
        for request in &all {
            let Value::Object(fields) = request_value(request) else {
                unreachable!()
            };
            for (name, _) in fields.iter().filter(|(n, _)| n != "operation") {
                if name == "beforeSequence" || name == "beforePublicationId" {
                    // One cursor field alone is the invalid case.
                    assert_eq!(
                        parse_request(&with(request, name, None)),
                        Err(Error::Invalid),
                        "{name}"
                    );
                    continue;
                }
                assert_eq!(
                    parse_request(&with(request, name, None)),
                    Err(Error::Invalid),
                    "missing {name}"
                );
                assert_eq!(
                    parse_request(&with(request, name, Some(Value::Bool(true)))),
                    Err(Error::Invalid),
                    "type {name}"
                );
            }
            let mut duplicate = fields.clone();
            duplicate.push(duplicate[1].clone());
            assert_eq!(
                parse_request(&Value::Object(duplicate)),
                Err(Error::Invalid)
            );
        }
        let store = &all[0];
        assert_eq!(
            parse_request(&with(store, "contentSize", Some(Value::Uint64(4)))),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_request(&with(store, "contentSize", Some(Value::Uint64(0)))),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_request(&with(store, "publisherId", Some(Value::Bytes(vec![1; 31])))),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_request(&with(
                store,
                "audience",
                Some(Value::Array(vec![Value::Bytes(vec![1; 31])]))
            )),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_request(&with(
                store,
                "audience",
                Some(Value::Array(vec![Value::Bool(true)]))
            )),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_request(&with(store, "kind", Some(Value::String("note".into())))),
            Err(Error::Unsupported)
        );
        assert_eq!(
            parse_request(&with(
                store,
                "visibility",
                Some(Value::String("secret".into()))
            )),
            Err(Error::Unsupported)
        );
        assert_eq!(
            parse_request(&with(&all[1], "limit", Some(Value::Uint32(0)))),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_request(&with(&all[1], "limit", Some(Value::Uint64(2)))),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_request(&with(&all[3], "maximumBytes", Some(Value::Uint64(0)))),
            Err(Error::Invalid)
        );
        assert_eq!(parse_request(&Value::Bool(true)), Err(Error::Invalid));
        let unknown = Value::Object(vec![("operation".into(), Value::String("check".into()))]);
        assert_eq!(parse_request(&unknown), Err(Error::Unsupported));
    }

    // A field that belongs only to another operation is rejected, while an unknown field is ignored (compatible extension).
    #[test]
    fn foreign_fields_are_rejected_and_unknown_ones_ignored() {
        let all = requests();
        let foreign = [
            (&all[0], "placementId", Value::Bytes(vec![1; 32])),
            (&all[1], "content", Value::Bytes(vec![1])),
            (&all[3], "limit", Value::Uint32(1)),
            (&all[4], "removalId", Value::Bytes(vec![1; 32])),
            (&all[5], "additionalSeconds", Value::Uint64(1)),
        ];
        for (request, name, value) in foreign {
            assert_eq!(
                parse_request(&with(request, name, Some(value))),
                Err(Error::Invalid),
                "{name}"
            );
        }
        for request in &all {
            let extended = with(request, "futureField", Some(Value::Bool(true)));
            assert_eq!(parse_request(&extended).as_ref(), Ok(request));
        }
    }

    // Responses with a malformed entry, wrong lengths, a missing price or an unknown operation are rejected.
    #[test]
    fn malformed_responses_are_rejected() {
        let good = response_value(&Response::List(
            ListOutput {
                publisher_id: IdentityId([2; 32]),
                entries: vec![],
                more: false,
            },
            1,
        ));
        let Value::Object(fields) = good else {
            unreachable!()
        };
        let without =
            |name: &str| Value::Object(fields.iter().filter(|(n, _)| n != name).cloned().collect());
        for name in ["publisherId", "entries", "more", "price"] {
            assert_eq!(
                parse_response(&without(name)),
                Err(Error::Invalid),
                "{name}"
            );
        }
        let bad_entry = Value::Object(
            fields
                .iter()
                .map(|(n, v)| match n.as_str() {
                    "entries" => (n.clone(), Value::Array(vec![Value::Object(vec![])])),
                    _ => (n.clone(), v.clone()),
                })
                .collect(),
        );
        assert_eq!(parse_response(&bad_entry), Err(Error::Invalid));
        let unknown = Value::Object(vec![("operation".into(), Value::String("recover".into()))]);
        assert_eq!(parse_response(&unknown), Err(Error::Unsupported));
        let get = response_value(&Response::Get(
            GetOutput {
                publication: publication(),
            },
            2,
        ));
        let Value::Object(mut get_fields) = get else {
            unreachable!()
        };
        get_fields.retain(|(n, _)| n != "contentHash");
        assert_eq!(
            parse_response(&Value::Object(get_fields)),
            Err(Error::Invalid)
        );
    }

    // Check challenges and evidence round trip, and evidence must be exactly one byte.
    #[test]
    fn check_values_round_trip_and_evidence_is_one_byte() {
        let input = CheckRequest {
            placement_id: PlacementId([4; 32]),
            offset: 2,
        };
        assert_eq!(parse_check_input(&check_input_value(&input)), Ok(input));
        let output = CheckOutput {
            placement_id: PlacementId([4; 32]),
            offset: 2,
            value: b'c',
        };
        assert_eq!(parse_check_output(&check_output_value(&output)), Ok(output));
        for value in [vec![], vec![1, 2]] {
            let bad = Value::Object(vec![
                ("placementId".into(), Value::Bytes(vec![4; 32])),
                ("offset".into(), Value::Uint64(2)),
                ("value".into(), Value::Bytes(value)),
            ]);
            assert_eq!(parse_check_output(&bad), Err(Error::Invalid));
        }
        assert_eq!(
            parse_check_input(&Value::Object(vec![])),
            Err(Error::Invalid)
        );
    }
}
