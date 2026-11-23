# Hosted Publications module guide

| Contract | Responsibility |
| --- | --- |
| [Requirements](REQUIREMENTS.md) | Mapping of HP-01 to HP-15 and PG-01 to PG-12 to owners, support and gaps |
| [Ids](ids.md) | Publication and content-hash identifiers; reused placement and removal IDs |
| [Metadata](metadata.md) | Kinds, visibility, publication metadata and the signed publication |
| [Signing](signing.md) | Exact signing bytes, publisher signing and the independent vectors |
| [Verify](verify.md) | Publisher, rule, size, hash and signature checks with bounds |
| [Envelope](envelope.md) | Restricted-content layout, sealing, structure check and opening |
| [Terms](terms.md) | Validated offering economics, limits and exact prices |
| [Placement](placement.md) | Placement states, record and content |
| [Store](store.md) | Durable bounded placement storage and host rules |
| [Lifecycle](lifecycle.md) | Expiration, removal and renewal transitions |
| [Operation](operation.md) | Typed operation inputs and outputs |
| [Provider](provider.md) | Assessing, pricing and committing operations |
| [Codec](codec.md) | Typed requests and responses over the Services value model |
| [Standard](standard.md) | Pinned definition and Services recognition |
| [Effect](effect.md) | Effects and outcomes with durable encoding |
| [Journal](journal.md) | Durable participant journal |
| [Participant](participant.md) | Prepare, commit, abort and status of effects |
| [Recovery](recovery.md) | Resolving prepared operations before admitting work |
| [Service](service/mod.md) | Services provider adapter |
| [Check](check.md) | Requester judgement of byte checks |
| [Error](error.md) | Typed failures and their Services mapping |

See the [module README](README.md) for purpose, status and relationships.
