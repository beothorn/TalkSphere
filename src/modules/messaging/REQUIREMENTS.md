# Messaging requirements reconciliation

Baseline: [CR2SE 73edf2d](https://github.com/beothorn/CR2SE/tree/73edf2dc230bccb0f27f2ea6e06e938fff738862), rechecked 2026-11-22. It differs from
the earlier pin bc53df4 only in the Peer Protocol `resource_limit` text (proposal [#13](https://github.com/beothorn/CR2SE/issues/13));
the Messaging specification, contracts, pseudocode and acceptance cases are unchanged. Sources:
[specification](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/messaging/README.md),
[inputs](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/messaging/inputs/README.md),
[outputs](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/messaging/outputs/README.md),
[acceptance cases MS-01 to MS-17](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/messaging/acceptanceTests/README.md).

## Payload bound of the finite profile

Messaging payloads travel as one detached byte value. The finite profile shares Storage's logical value budget (depth 12, 4096
nodes, 65536 bytes), so a payload is at most **16384 bytes** in this profile, the same limit as Storage objects; the offering's
`maximumPayloadBytes` may not exceed it and the module refuses larger values before allocation. Real applications split larger content
(an encrypted payload adds 105 bytes of header and tag).

## Acceptance cases to owners

| Case | Rule | Owner | Existing support | Planned step |
| --- | --- | --- | --- | --- |
| MS-01 | exact message signing bytes, Ed25519 over them, any mutation fails | `messaging::signing` | Cryptography `signature::sign/verify`, Identity ID derivation | 8 |
| MS-02 | durable placement, acceptedAt/expiresAt, price 17, restart, second store is a new placement | `messaging::placement`, `store` | Storage's durable-before-success pattern | 12, 17 |
| MS-03 | store rejection list; price rounding; zero denominators | `messaging::{pricing, terms, input}` | Storage `pricing`/`terms` as model | 5, 6, 7, 8 |
| MS-04 | role authorization and indistinguishable denials | `messaging::authorization` | Services `Peer` carries the authenticated identity | 18-22, 29 |
| MS-05 | discovery order, exclusive pair cursor, empty page charged | `messaging::discover` | none | 18 |
| MS-06 | recovery request binding | `messaging::signing` | Cryptography | 9 |
| MS-07 | recovery output and signed provider receipt | `messaging::recover` | Cryptography, provider key via Identity | 19 |
| MS-08 | recovery retry returns cached output, no new charge | `messaging::recover` + Services `replay` | Services `replay::Id` and Peer Protocol `replay.rs` already carry Messaging replay IDs | 20 |
| MS-09 | sender status with both signatures | `messaging::status` | none | 21 |
| MS-10 | guarded renewal, one winner | `messaging::renew` | Storage renewal guard as model | 22 |
| MS-11, MS-12 | release-credit quote, atomic removal, replay for the ledger lifetime | `messaging::remove` | Peer Protocol binds `releaseCredits` in the quote; **Ledger `grant` exists but is not part of a Services decision** | 23-25 |
| MS-13 | paid byte evidence and local pass/fail/inconclusive | `messaging::check` | Storage `byte_check` and sampling | 26, 27 |
| MS-14 | `cr2se.identity-encrypted.v1` payload | `messaging::encryption` | Cryptography X25519/HKDF/XChaCha20 and an independent fixture (`fixtures/messaging.md`) | 10 |
| MS-15 | Board `identityEncryption` credential | `messaging::credential`, Board | **Board has no such field** | 11 |
| MS-16 | races and expiry | `messaging::placement` | none | 13, 34 |
| MS-17 | replica identity, independence from paid Storage and Discovery | `messaging` | none | 32 |

## Gaps found

1. **Release credits are not one decision.** Removal charges `removePrice` and issues `releaseCredits` as two Ledger records that must apply
   once together. Services settles a charge through its coordinator; the issuance is only `Ledger::grant`, an administrative call that is
   not idempotent. Step 24 resolved this: the Ledger now has `issue_release`, idempotent per operation key, and the participant issues the credits when
   the removal commits, so recovery completes an interrupted settlement exactly once.
2. **The Board has no `identityEncryption` field**; step 11 adds it as an optional, independently validated top-level field.
3. **No Messaging standard definition is recognized** by Services: step 4 imports the pinned definition and a catalogue like Storage's.
4. **The provider needs its own signing key** for recovery receipts; Identity exposes signing only through scoped capabilities, so the
   module takes a `Signer` port and the node supplies it.

## Rules the module enforces itself (no peer text, exact values)

Exact fixed lengths (IDs 32, keys 32, signatures 64), `payloadSize >= 1`, SHA-256 payload hash, the two payload encodings, retention within
offering limits with overflow-safe times, one-time recovery and removal IDs, serialized per-placement transitions, no charge for failed
operations, unauthorized replies that reveal nothing, and finite bounds on every table.
