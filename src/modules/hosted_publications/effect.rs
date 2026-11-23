//! Hosted publication effects proposed to the participant and the outcomes they produce.
//!
//! Callers: the participant and its journal. An effect is the exact state change one accepted operation will make; it is prepared durably
//! before the coordinator decides and applied at most once on commit. Outcomes are what the requester is told, retained in the journal so
//! a lost reply can be answered from the original result. The encoding is the participant's own durable format, never a peer wire format.
use super::{
    error::Error,
    ids::{ContentHash, PlacementId, PublicationId, RemovalId, Signature},
    metadata::{Kind, Metadata, Publication, Visibility},
    operation::RemoveRequest,
};
use crate::modules::{
    identity::text,
    messaging::file_io::{hex, unhex, unhex_vec},
};
use serde_json::{Value, json};

/// The state change of one accepted operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// A read-only operation (list, get): nothing changes. The encoded request is kept so a lost staged result can be produced again
    /// after a restart.
    Query {
        /// The request value in the Services value encoding.
        request: Vec<u8>,
    },
    /// Create a pending placement for a verified publication.
    Store {
        /// The host-chosen placement identifier, fixed at preparation so a retry is the same placement.
        placement_id: PlacementId,
        /// The verified signed publication, boxed because it is far larger than the other effects.
        publication: Box<Publication>,
        /// Requested initial retention.
        retention_seconds: u64,
        /// The agreed price.
        price: u64,
    },
    /// Extend a placement.
    Renew {
        /// The placement.
        placement_id: PlacementId,
        /// The expiry the publisher saw.
        expected_expires_at: u64,
        /// Seconds to add.
        additional_seconds: u64,
        /// Latest expiry the offering allows.
        ceiling_expires_at: u64,
        /// The agreed price.
        price: u64,
    },
    /// End a pending placement and issue release credits.
    Remove {
        /// The verified removal request.
        request: RemoveRequest,
        /// The agreed early-release credits.
        release_credits: u64,
        /// The agreed removal charge.
        price: u64,
    },
}

/// The result of a committed effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A read-only operation has no outcome of its own.
    Query,
    /// A placement was created.
    Stored {
        /// The placement.
        placement_id: PlacementId,
        /// The stored publication.
        publication_id: PublicationId,
        /// Host acceptance time, assigned at the durable commit.
        accepted_at: u64,
        /// Acceptance time plus the retention.
        expires_at: u64,
        /// Credits charged.
        price: u64,
    },
    /// A renewal was applied.
    Renewed {
        /// The placement.
        placement_id: PlacementId,
        /// Expiry before the renewal.
        previous_expires_at: u64,
        /// Expiry after it.
        expires_at: u64,
        /// Credits charged.
        price: u64,
    },
    /// A removal was applied.
    Removed {
        /// The placement.
        placement_id: PlacementId,
        /// The removal identifier.
        removal_id: RemovalId,
        /// Credits issued back.
        release_credits: u64,
        /// Credits charged.
        price: u64,
    },
}

impl Effect {
    /// Encode as the participant's durable JSON.
    pub fn encode(&self) -> Vec<u8> {
        tracing::trace!("Encoding a hosted publication effect");
        let value = match self {
            Self::Query { request } => json!({"kind": "query", "request": hex(request)}),
            Self::Store {
                placement_id,
                publication,
                retention_seconds,
                price,
            } => {
                let m = &publication.metadata;
                json!({"kind": "store", "placementId": hex(&placement_id.0), "retention": retention_seconds, "price": price,
                    "publicationId": hex(&m.publication_id.0), "publisherId": text::format(m.publisher_id),
                    "contentKind": m.kind.name(), "sequence": m.sequence, "publishedAt": m.published_at,
                    "visibility": m.visibility.name(),
                    "audience": m.audience.iter().map(|id| text::format(*id)).collect::<Vec<_>>(),
                    "contentSize": m.content_size, "contentHash": hex(&m.content_hash.0),
                    "publisherKey": hex(&publication.publisher_public_key),
                    "publisherSignature": hex(&publication.publisher_signature.0),
                    "content": hex(&publication.content)})
            }
            Self::Renew {
                placement_id,
                expected_expires_at,
                additional_seconds,
                ceiling_expires_at,
                price,
            } => json!({
                "kind": "renew", "placementId": hex(&placement_id.0), "expected": expected_expires_at,
                "additional": additional_seconds, "ceiling": ceiling_expires_at, "price": price}),
            Self::Remove {
                request,
                release_credits,
                price,
            } => json!({"kind": "remove", "price": price, "release": release_credits,
                "placementId": hex(&request.placement_id.0), "removalId": hex(&request.removal_id.0),
                "expected": request.expected_expires_at, "minimum": request.minimum_release_credits}),
        };
        serde_json::to_vec(&value).unwrap_or_default()
    }

    /// Decode durable JSON strictly.
    /// # Errors
    /// `Storage` for anything malformed (a corrupt journal is never guessed at).
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        tracing::trace!("Decoding a hosted publication effect");
        let value: Value = serde_json::from_slice(bytes).map_err(|_| Error::Storage)?;
        let o = value.as_object().ok_or(Error::Storage)?;
        let text_of = |n: &str| o.get(n).and_then(Value::as_str);
        let number = |n: &str| o.get(n).and_then(Value::as_u64).ok_or(Error::Storage);
        let bad = || Error::Storage;
        let id = |n: &str| text_of(n).and_then(unhex::<32>).ok_or(Error::Storage);
        Ok(match text_of("kind").ok_or_else(bad)? {
            "query" => Self::Query {
                request: text_of("request").and_then(unhex_vec).ok_or_else(bad)?,
            },
            "store" => {
                let audience = o
                    .get("audience")
                    .and_then(Value::as_array)
                    .ok_or_else(bad)?
                    .iter()
                    .map(|v| text::parse(v.as_str()?).ok())
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(bad)?;
                Self::Store {
                    placement_id: PlacementId(id("placementId")?),
                    retention_seconds: number("retention")?,
                    price: number("price")?,
                    publication: Box::new(Publication {
                        metadata: Metadata {
                            publication_id: PublicationId(id("publicationId")?),
                            publisher_id: text::parse(text_of("publisherId").ok_or_else(bad)?)
                                .map_err(|_| bad())?,
                            kind: Kind::from_name(text_of("contentKind").ok_or_else(bad)?)
                                .map_err(|_| bad())?,
                            sequence: number("sequence")?,
                            published_at: number("publishedAt")?,
                            visibility: Visibility::from_name(
                                text_of("visibility").ok_or_else(bad)?,
                            )
                            .map_err(|_| bad())?,
                            audience,
                            content_size: number("contentSize")?,
                            content_hash: ContentHash(id("contentHash")?),
                        },
                        publisher_public_key: id("publisherKey")?,
                        publisher_signature: Signature(
                            text_of("publisherSignature")
                                .and_then(unhex::<64>)
                                .ok_or_else(bad)?,
                        ),
                        content: text_of("content").and_then(unhex_vec).ok_or_else(bad)?,
                    }),
                }
            }
            "renew" => Self::Renew {
                placement_id: PlacementId(id("placementId")?),
                expected_expires_at: number("expected")?,
                additional_seconds: number("additional")?,
                ceiling_expires_at: number("ceiling")?,
                price: number("price")?,
            },
            "remove" => Self::Remove {
                price: number("price")?,
                release_credits: number("release")?,
                request: RemoveRequest {
                    placement_id: PlacementId(id("placementId")?),
                    removal_id: RemovalId(id("removalId")?),
                    expected_expires_at: number("expected")?,
                    minimum_release_credits: number("minimum")?,
                },
            },
            _ => return Err(Error::Storage),
        })
    }

    /// The placement this effect concerns, when it names one.
    pub fn placement(&self) -> Option<PlacementId> {
        match self {
            Self::Query { .. } => None,
            Self::Store { placement_id, .. } | Self::Renew { placement_id, .. } => {
                Some(*placement_id)
            }
            Self::Remove { request, .. } => Some(request.placement_id),
        }
    }
}

impl Outcome {
    /// Encode as durable JSON.
    pub fn encode(&self) -> Vec<u8> {
        tracing::trace!("Encoding a hosted publication outcome");
        let value = match self {
            Self::Query => json!({"kind": "query"}),
            Self::Stored {
                placement_id,
                publication_id,
                accepted_at,
                expires_at,
                price,
            } => json!({"kind": "stored",
                "placementId": hex(&placement_id.0), "publicationId": hex(&publication_id.0), "acceptedAt": accepted_at,
                "expiresAt": expires_at, "price": price}),
            Self::Renewed {
                placement_id,
                previous_expires_at,
                expires_at,
                price,
            } => json!({"kind": "renewed",
                "placementId": hex(&placement_id.0), "previous": previous_expires_at, "expiresAt": expires_at, "price": price}),
            Self::Removed {
                placement_id,
                removal_id,
                release_credits,
                price,
            } => json!({"kind": "removed",
                "placementId": hex(&placement_id.0), "removalId": hex(&removal_id.0), "release": release_credits, "price": price}),
        };
        serde_json::to_vec(&value).unwrap_or_default()
    }

    /// Decode durable JSON strictly.
    /// # Errors
    /// `Storage` for anything malformed.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        tracing::trace!("Decoding a hosted publication outcome");
        let value: Value = serde_json::from_slice(bytes).map_err(|_| Error::Storage)?;
        let o = value.as_object().ok_or(Error::Storage)?;
        let text_of = |n: &str| o.get(n).and_then(Value::as_str);
        let number = |n: &str| o.get(n).and_then(Value::as_u64).ok_or(Error::Storage);
        let id = |n: &str| text_of(n).and_then(unhex::<32>).ok_or(Error::Storage);
        Ok(match text_of("kind").ok_or(Error::Storage)? {
            "query" => Self::Query,
            "stored" => Self::Stored {
                placement_id: PlacementId(id("placementId")?),
                publication_id: PublicationId(id("publicationId")?),
                accepted_at: number("acceptedAt")?,
                expires_at: number("expiresAt")?,
                price: number("price")?,
            },
            "renewed" => Self::Renewed {
                placement_id: PlacementId(id("placementId")?),
                previous_expires_at: number("previous")?,
                expires_at: number("expiresAt")?,
                price: number("price")?,
            },
            "removed" => Self::Removed {
                placement_id: PlacementId(id("placementId")?),
                removal_id: RemovalId(id("removalId")?),
                release_credits: number("release")?,
                price: number("price")?,
            },
            _ => return Err(Error::Storage),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::identity::api::IdentityId;

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

    fn effects() -> Vec<Effect> {
        vec![
            Effect::Query {
                request: vec![1, 2, 3],
            },
            Effect::Store {
                placement_id: PlacementId([8; 32]),
                publication: Box::new(publication()),
                retention_seconds: 10,
                price: 17,
            },
            Effect::Renew {
                placement_id: PlacementId([8; 32]),
                expected_expires_at: 1010,
                additional_seconds: 10,
                ceiling_expires_at: 1200,
                price: 15,
            },
            Effect::Remove {
                request: RemoveRequest {
                    placement_id: PlacementId([8; 32]),
                    removal_id: RemovalId([9; 32]),
                    expected_expires_at: 1010,
                    minimum_release_credits: 4,
                },
                release_credits: 5,
                price: 2,
            },
        ]
    }

    // Every effect and outcome survives its durable encoding unchanged, and effects name the placement they concern.
    #[test]
    fn effects_and_outcomes_round_trip() {
        for effect in effects() {
            assert_eq!(Effect::decode(&effect.encode()), Ok(effect.clone()));
        }
        let placements: Vec<_> = effects().iter().map(Effect::placement).collect();
        assert_eq!(placements[0], None);
        assert!(
            placements[1..]
                .iter()
                .all(|p| *p == Some(PlacementId([8; 32])))
        );
        for outcome in [
            Outcome::Query,
            Outcome::Stored {
                placement_id: PlacementId([8; 32]),
                publication_id: PublicationId([1; 32]),
                accepted_at: 1000,
                expires_at: 1010,
                price: 17,
            },
            Outcome::Renewed {
                placement_id: PlacementId([8; 32]),
                previous_expires_at: 1010,
                expires_at: 1020,
                price: 15,
            },
            Outcome::Removed {
                placement_id: PlacementId([8; 32]),
                removal_id: RemovalId([9; 32]),
                release_credits: 5,
                price: 2,
            },
        ] {
            assert_eq!(Outcome::decode(&outcome.encode()), Ok(outcome.clone()));
        }
    }

    // Anything malformed in a stored effect or outcome is `Storage`, never guessed at: bad JSON, an unknown kind, a missing or mistyped
    // member, an invalid identifier, kind, visibility or audience entry.
    #[test]
    fn corrupt_encodings_are_storage_errors() {
        for bad in [
            b"nonsense".to_vec(),
            b"[]".to_vec(),
            br#"{"kind":"unknown"}"#.to_vec(),
            br#"{"kind":"query"}"#.to_vec(),
            br#"{"kind":"renew"}"#.to_vec(),
        ] {
            assert_eq!(Effect::decode(&bad), Err(Error::Storage));
        }
        assert_eq!(Outcome::decode(b"{}"), Err(Error::Storage));
        assert_eq!(
            Outcome::decode(br#"{"kind":"stored"}"#),
            Err(Error::Storage)
        );
        let good = effects()[1].encode();
        let text = String::from_utf8(good).unwrap();
        for (from, to) in [
            ("\"contentKind\":\"feed-post\"", "\"contentKind\":\"note\""),
            ("\"visibility\":\"restricted\"", "\"visibility\":\"secret\""),
            ("\"price\":17", "\"price\":\"17\""),
            (
                &format!("\"contentHash\":\"{}\"", hex(&[5; 32])),
                "\"contentHash\":\"zz\"",
            ),
        ] {
            assert!(text.contains(from), "{from}");
            assert_eq!(
                Effect::decode(text.replace(from, to).as_bytes()),
                Err(Error::Storage),
                "{from}"
            );
        }
        let broken_audience = text.replace(&text::format(IdentityId([3; 32])), "not an id");
        assert_eq!(
            Effect::decode(broken_audience.as_bytes()),
            Err(Error::Storage)
        );
    }
}
