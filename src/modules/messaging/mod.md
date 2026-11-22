# Messaging module guide

| Contract | Responsibility |
| --- | --- |
| [Requirements](REQUIREMENTS.md) | Mapping of MS-01 to MS-17 to owners, support and gaps |
| [Ids](ids.md) | Fixed-length identifiers, hashes and signatures |
| [Error](error.md) | Typed failures and their Services mapping |
| [State](state.md) | Placement states, payload encodings and constants |
| [Metadata](metadata.md) | Provider-visible metadata and the signed message |
| [Operation](operation.md) | Inputs and outputs of the six operations and the check |
| [Ports](ports.md) | Clock, entropy and signer supplied by composition |
| [Signing](signing.md) | Message signing bytes, sender signing and verification |
| [Placement](placement.md) | A placement record and its payload |
| [Store](store.md) | Durable bounded placement storage |
| [Lifecycle](lifecycle.md) | Expiration, transitions and per-placement serialization |
| [Effect](effect.md) | Effects and outcomes with durable encoding |
| [Journal](journal.md) | Durable participant journal |
| [Participant](participant.md) | Prepare, commit, abort and status of effects |
| [Recovery](recovery.md) | Resolving prepared operations before admitting work |
| [Check](check.md) | Requester judgement of byte checks from private expectations |
| [Provider](provider.md) | Store assessment before the body and durable commit |
| [Receive](receive.md) | Authenticating, deduplicating and decrypting recovered messages |
| [Reclaim](reclaim.md) | Freeing finished placements without losing what is owed |
| [Staging](staging.md) | Bounded reservations for incoming payloads |
| [Credential](credential.md) | Board `identityEncryption` lookup and verification |
| [Encryption](encryption.md) | The identity-encrypted payload format |
| [Receipt](receipt.md) | Recovery request and provider receipt signatures |
| [Codec](codec.md) | Typed requests and responses over the Services value model |
| [Terms](terms.md) | Validated offering economics and limits |
| [Pricing](pricing.md) | Exact rational store, recover and renew prices |
| [Standard](standard.md) | Pinned definition and Services recognition |
| [Fixtures](fixtures/README.md) | Independently generated schema and byte vectors |
| [Test doubles](test_support.md) | Deterministic clock, entropy and identities for tests |

See the [module README](README.md) for purpose, status and relationships.
