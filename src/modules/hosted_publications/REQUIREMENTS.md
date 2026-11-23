# Hosted Publications requirements reconciliation

Baseline: [CR2SE 53f1245](https://github.com/beothorn/CR2SE/tree/53f12451859e0ad207ade9b8a23be20ceb404495), the Hosted Publications proposal
([beothorn/CR2SE#14](https://github.com/beothorn/CR2SE/issues/14), not yet accepted upstream) over the existing modules at
[73edf2d](https://github.com/beothorn/CR2SE/tree/73edf2dc230bccb0f27f2ea6e06e938fff738862). Sources:
[specification](https://github.com/beothorn/CR2SE/blob/53f12451859e0ad207ade9b8a23be20ceb404495/modules/hosted-publications/README.md),
[vectors](https://github.com/beothorn/CR2SE/blob/53f12451859e0ad207ade9b8a23be20ceb404495/modules/hosted-publications/vectors/README.md),
[acceptance cases HP-01 to HP-15](https://github.com/beothorn/CR2SE/blob/53f12451859e0ad207ade9b8a23be20ceb404495/modules/hosted-publications/acceptanceTests/README.md),
[Static Pages cases PG-01 to PG-12](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/pages/acceptanceTests/README.md).
If upstream changes the proposal before accepting it, recheck every step.

## Acceptance cases to owners

| Case | Rule | Owner | Existing support | Plan step |
| --- | --- | --- | --- | --- |
| HP-01 | signing bytes, vectors V1-V3, mutation fails | `hosted_publications::signing` | Cryptography `signature`, Identity ID derivation | 3 |
| HP-02 | durable placement, exact stored bytes, restart | `placement`, `store` | Messaging's durable-before-success pattern | 7, 8 |
| HP-03 | store rejection list, no charge | `provider`, `verify` | Messaging `pricing`, Services charging | 4, 8 |
| HP-04 | visibility, indistinguishable hiding | `provider` (list, get) | none | 9, 10 |
| HP-05 | list order, exclusive pair cursor | `provider` (list) | Messaging discover as a model | 9 |
| HP-06 | get bounds and price | `provider` (get), `pricing` | Messaging `pricing::usage` | 6, 10 |
| HP-07 | envelope vector, opening failures | `envelope` | Cryptography X25519, HKDF, XChaCha20 | 5 |
| HP-08 | hosted page assembly | `pages::hosted` | Static Pages paths and media types | 13, 14 |
| HP-09 | hosted feed post | milestone 16 (`public_feeds`) | none | out of scope |
| HP-10 | versions and equivocation | `pages::latest` | none | 15 |
| HP-11 | renew | `provider` | Messaging renew rules | 11 |
| HP-12 | remove, release credits, replay | `provider` | Ledger `issue_release`, Messaging participant | 11, 12 |
| HP-13 | check | `provider`, requester `check` | Messaging `check` | 11 |
| HP-14 | authorization of renew/remove/check | `provider` | Services peer identity | 11 |
| HP-15 | definitions, independence of replicas | `standard`, Services adapter | Messaging `standard` | 6, 12, 19 |
| PG-01..12 | publisher-served `cr2se.page` | `pages::service` | Services provider contracts | 13, 16 |

## Content bound of the finite profile

Services values share a logical budget of depth 12, 4096 nodes and 65536 bytes, so publication content is at most **16384 bytes** in this
profile, the same limit as a Messaging payload and a Storage object. An offering's `maximumContentBytes` may not exceed it and the module
refuses larger values before allocating. The upstream example offering advertises 10000000 bytes; a conforming node of this profile
advertises at most 16384. Real pages and posts are split into resources and posts below this size (a restricted envelope adds 61 bytes
plus 104 per reader and a 16-byte tag).

## Gaps found

1. **Messaging's participant is Messaging-specific.** Hosted Publications reuses Messaging's rules by reference and its public pricing
   helpers and ports, but owns its placement, journal and effect types, so neither module reads the other's private state.
2. **HP-09 needs Public Feeds canonical message bytes**, which belong to milestone 16. Milestone 15 stores any `feed-post` content as
   opaque verified bytes; the feed-specific checks arrive with the feeds module.
3. **HP-05 seeds two feed posts at sequence 7**, but section 7 makes a host reject a second pending publication at the same sequence, so the
   case cannot be built through `store`, and the `(sequence, publicationId)` tie-break of list can never apply at one host. Implemented as
   written (the store refuses the pair); the tie-break key is kept. To be confirmed upstream before filing a protocol report.
4. **Restricted resources in a manifest.** Section 11 says a manifest entry holds the SHA-256 of the resource content and the assembly rule says the publication's `contentSize` and
   `contentHash` equal the entry; for a restricted resource the publication's content is the envelope, so TalkSphere sizes and hashes the stored (encrypted) bytes, which is the only reading that
   makes both rules hold. To be clarified upstream.
5. **No Board change.** The audience's encryption keys are found through the existing `identityEncryption` field.

## Rules the module enforces itself

Exact fixed lengths (IDs, keys, hashes 32; signatures 64), big-endian signing inputs, `contentSize >= 1`, SHA-256 content hash, ascending
unique audience, the envelope layout of exactly 61 + 104 per entry + at least 16 bytes, resource sequence zero, finite
tables, idempotent removal, no charge for failed operations and concealed denials.
