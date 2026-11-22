//! Messaging effects proposed to the participant and the outcomes they produce.
//!
//! Callers: the participant and its journal. An effect is the exact state change one accepted operation will make; it is
//! prepared durably before the coordinator decides and applied at most once on commit. Outcomes are what the requester
//! is told, retained in the journal so a lost reply can be answered from the original result. The encoding is the
//! participant's own durable format, never a peer wire format.
use super::{
    error::Error,
    file_io::{hex, unhex, unhex_vec},
    ids::{MessageId, PayloadHash, PlacementId, RecoveryId, RemovalId, Signature},
    metadata::{Metadata, SignedMessage},
    operation::{Receipt, RecoverInput, RemoveInput},
    state::Encoding,
};
use crate::modules::identity::text;
use serde_json::{Value, json};

/// The state change of one accepted operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// A read-only operation (discover, status): nothing changes. The encoded request is kept so a lost staged result can be produced
    /// again after a restart.
    Query {
        /// The request value in the Services value encoding.
        request: Vec<u8>,
    },
    /// Create a pending placement for a verified message.
    Store {
        /// The provider-chosen placement identifier, fixed at preparation so a retry is the same placement.
        placement_id: PlacementId,
        /// The verified signed message.
        message: SignedMessage,
        /// Requested initial retention.
        retention_seconds: u64,
        /// The agreed price.
        price: u64,
    },
    /// Recover a pending placement for its recipient.
    Recover {
        /// The verified recovery request.
        input: RecoverInput,
        /// The agreed price.
        price: u64,
    },
    /// Extend a placement.
    Renew {
        /// The placement.
        placement_id: PlacementId,
        /// The expiry the sender saw.
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
        input: RemoveInput,
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
        /// The stored message.
        message_id: MessageId,
        /// Provider acceptance time, assigned at the durable commit.
        accepted_at: u64,
        /// Acceptance time plus the retention.
        expires_at: u64,
        /// Credits charged.
        price: u64,
    },
    /// A recovery was recorded.
    Recovered {
        /// The placement.
        placement_id: PlacementId,
        /// The recorded receipt.
        receipt: Receipt,
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
        tracing::trace!("Encoding a Messaging effect");
        let value = match self {
            Self::Query { request } => json!({"kind": "query", "request": hex(request)}),
            Self::Store {
                placement_id,
                message,
                retention_seconds,
                price,
            } => {
                let m = &message.metadata;
                json!({"kind": "store", "placementId": hex(&placement_id.0), "retention": retention_seconds, "price": price,
                    "messageId": hex(&m.message_id.0), "senderId": text::format(m.sender_id), "recipientId": text::format(m.recipient_id),
                    "createdAt": m.sender_created_at, "encoding": m.encoding.name(), "payloadSize": m.payload_size,
                    "payloadHash": hex(&m.payload_hash.0), "senderKey": hex(&message.sender_public_key),
                    "senderSignature": hex(&message.sender_signature.0), "payload": hex(&message.payload)})
            }
            Self::Recover { input, price } => {
                json!({"kind": "recover", "price": price, "placementId": hex(&input.placement_id.0),
                "messageId": hex(&input.message_id.0), "payloadHash": hex(&input.payload_hash.0), "recoveryId": hex(&input.recovery_id.0),
                "recipientKey": hex(&input.recipient_public_key), "recipientSignature": hex(&input.recipient_signature.0)})
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
                input,
                release_credits,
                price,
            } => json!({"kind": "remove", "price": price, "release": release_credits,
                "placementId": hex(&input.placement_id.0), "removalId": hex(&input.removal_id.0),
                "expected": input.expected_expires_at, "minimum": input.minimum_release_credits}),
        };
        serde_json::to_vec(&value).unwrap_or_default()
    }

    /// Decode durable JSON strictly.
    /// # Errors
    /// `Storage` for anything malformed (a corrupt journal is never guessed at).
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        tracing::trace!("Decoding a Messaging effect");
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
                let payload = text_of("payload").and_then(unhex_vec).ok_or_else(bad)?;
                Self::Store {
                    placement_id: PlacementId(id("placementId")?),
                    retention_seconds: number("retention")?,
                    price: number("price")?,
                    message: SignedMessage {
                        metadata: Metadata {
                            message_id: MessageId(id("messageId")?),
                            sender_id: text::parse(text_of("senderId").ok_or_else(bad)?)
                                .map_err(|_| bad())?,
                            recipient_id: text::parse(text_of("recipientId").ok_or_else(bad)?)
                                .map_err(|_| bad())?,
                            sender_created_at: number("createdAt")?,
                            encoding: Encoding::parse(text_of("encoding").ok_or_else(bad)?)
                                .ok_or_else(bad)?,
                            payload_size: number("payloadSize")?,
                            payload_hash: PayloadHash(id("payloadHash")?),
                        },
                        sender_public_key: id("senderKey")?,
                        sender_signature: Signature(
                            text_of("senderSignature")
                                .and_then(unhex::<64>)
                                .ok_or_else(bad)?,
                        ),
                        payload,
                    },
                }
            }
            "recover" => Self::Recover {
                price: number("price")?,
                input: RecoverInput {
                    placement_id: PlacementId(id("placementId")?),
                    message_id: MessageId(id("messageId")?),
                    payload_hash: PayloadHash(id("payloadHash")?),
                    recovery_id: RecoveryId(id("recoveryId")?),
                    recipient_public_key: id("recipientKey")?,
                    recipient_signature: Signature(
                        text_of("recipientSignature")
                            .and_then(unhex::<64>)
                            .ok_or_else(bad)?,
                    ),
                },
            },
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
                input: RemoveInput {
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
            Self::Recover { input, .. } => Some(input.placement_id),
            Self::Remove { input, .. } => Some(input.placement_id),
        }
    }
}

impl Outcome {
    /// Encode as durable JSON.
    pub fn encode(&self) -> Vec<u8> {
        tracing::trace!("Encoding a Messaging outcome");
        let value = match self {
            Self::Query => json!({"kind": "query"}),
            Self::Stored {
                placement_id,
                message_id,
                accepted_at,
                expires_at,
                price,
            } => json!({"kind": "stored",
                "placementId": hex(&placement_id.0), "messageId": hex(&message_id.0), "acceptedAt": accepted_at,
                "expiresAt": expires_at, "price": price}),
            Self::Recovered {
                placement_id,
                receipt,
                price,
            } => json!({"kind": "recovered", "placementId": hex(&placement_id.0),
                "price": price, "recoveryId": hex(&receipt.recovery_id.0), "recoveredAt": receipt.recovered_at,
                "recipientKey": hex(&receipt.recipient_public_key), "recipientSignature": hex(&receipt.recipient_signature.0),
                "providerSignature": hex(&receipt.provider_signature.0)}),
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
        tracing::trace!("Decoding a Messaging outcome");
        let value: Value = serde_json::from_slice(bytes).map_err(|_| Error::Storage)?;
        let o = value.as_object().ok_or(Error::Storage)?;
        let text_of = |n: &str| o.get(n).and_then(Value::as_str);
        let number = |n: &str| o.get(n).and_then(Value::as_u64).ok_or(Error::Storage);
        let id = |n: &str| text_of(n).and_then(unhex::<32>).ok_or(Error::Storage);
        Ok(match text_of("kind").ok_or(Error::Storage)? {
            "query" => Self::Query,
            "stored" => Self::Stored {
                placement_id: PlacementId(id("placementId")?),
                message_id: MessageId(id("messageId")?),
                accepted_at: number("acceptedAt")?,
                expires_at: number("expiresAt")?,
                price: number("price")?,
            },
            "recovered" => Self::Recovered {
                placement_id: PlacementId(id("placementId")?),
                price: number("price")?,
                receipt: Receipt {
                    recovery_id: RecoveryId(id("recoveryId")?),
                    recovered_at: number("recoveredAt")?,
                    recipient_public_key: id("recipientKey")?,
                    recipient_signature: Signature(
                        text_of("recipientSignature")
                            .and_then(unhex::<64>)
                            .ok_or(Error::Storage)?,
                    ),
                    provider_signature: Signature(
                        text_of("providerSignature")
                            .and_then(unhex::<64>)
                            .ok_or(Error::Storage)?,
                    ),
                },
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

    fn message() -> SignedMessage {
        SignedMessage {
            metadata: Metadata {
                message_id: MessageId([1; 32]),
                sender_id: IdentityId([2; 32]),
                recipient_id: IdentityId([3; 32]),
                sender_created_at: 1000,
                encoding: Encoding::IdentityEncryptedV1,
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

    // Every effect and outcome survives encode and decode exactly, including the maxima of every integer.
    #[test]
    fn effects_and_outcomes_round_trip() {
        let p = PlacementId([11; 32]);
        let effects = [
            Effect::Query {
                request: vec![1, 2, 3],
            },
            Effect::Store {
                placement_id: p,
                message: message(),
                retention_seconds: u64::MAX,
                price: 17,
            },
            Effect::Recover {
                price: 2,
                input: RecoverInput {
                    placement_id: p,
                    message_id: MessageId([1; 32]),
                    payload_hash: PayloadHash([4; 32]),
                    recovery_id: RecoveryId([7; 32]),
                    recipient_public_key: [8; 32],
                    recipient_signature: Signature([9; 64]),
                },
            },
            Effect::Renew {
                placement_id: p,
                expected_expires_at: 1010,
                additional_seconds: 4,
                ceiling_expires_at: u64::MAX,
                price: 6,
            },
            Effect::Remove {
                price: 2,
                release_credits: 5,
                input: RemoveInput {
                    placement_id: p,
                    removal_id: RemovalId([12; 32]),
                    expected_expires_at: 1010,
                    minimum_release_credits: 4,
                },
            },
        ];
        for effect in effects {
            assert_eq!(Effect::decode(&effect.encode()), Ok(effect));
        }
        let outcomes = [
            Outcome::Query,
            Outcome::Stored {
                placement_id: p,
                message_id: MessageId([1; 32]),
                accepted_at: 1000,
                expires_at: 1010,
                price: 17,
            },
            Outcome::Recovered {
                placement_id: p,
                receipt: receipt(),
                price: 2,
            },
            Outcome::Renewed {
                placement_id: p,
                previous_expires_at: 1010,
                expires_at: 1014,
                price: 6,
            },
            Outcome::Removed {
                placement_id: p,
                removal_id: RemovalId([12; 32]),
                release_credits: 5,
                price: 2,
            },
        ];
        for outcome in outcomes {
            assert_eq!(Outcome::decode(&outcome.encode()), Ok(outcome));
        }
    }

    // Malformed, unknown-kind and truncated durable bytes are storage corruption, never guessed at.
    #[test]
    fn corrupt_durable_bytes_are_refused() {
        for bad in [
            &b"nonsense"[..],
            b"[]",
            br#"{"kind":"launch"}"#,
            br#"{"kind":"store"}"#,
            br#"{"kind":"renew","placementId":"zz"}"#,
        ] {
            assert_eq!(Effect::decode(bad), Err(Error::Storage));
            assert_eq!(Outcome::decode(bad), Err(Error::Storage));
        }
        let good = Effect::Store {
            placement_id: PlacementId([1; 32]),
            message: message(),
            retention_seconds: 1,
            price: 1,
        }
        .encode();
        assert_eq!(Effect::decode(&good[..good.len() - 3]), Err(Error::Storage));
    }

    // An effect names the placement it concerns, and a query names none.
    #[test]
    fn effects_name_their_placement() {
        let p = PlacementId([11; 32]);
        assert_eq!(Effect::Query { request: vec![] }.placement(), None);
        assert_eq!(
            Effect::Renew {
                placement_id: p,
                expected_expires_at: 1,
                additional_seconds: 1,
                ceiling_expires_at: 1,
                price: 1
            }
            .placement(),
            Some(p)
        );
    }
}
