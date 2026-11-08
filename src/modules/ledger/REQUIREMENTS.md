# Ledger requirements and evidence map

Baseline: [d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
Rechecked 2026-11-06: main is six commits ahead; no Ledger or common contract changes.

Authority: [specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/ledger/README.md),
[inputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/ledger/inputs/README.md),
[outputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/ledger/outputs/README.md),
[pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/ledger/pseudocode/README.md),
[acceptance suite](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/ledger/acceptanceTests/README.md),
and [common recovery contract](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md).

Every row below is mapped to executed assertions in [ACCEPTANCE.md](ACCEPTANCE.md);
requirements review alone is not conformance evidence. Real Services/network
composition obligations remain explicitly deferred in that evidence.
Split mutation lists into individual assertions when implementing their owning step.

| Case | Observable assertions | Steps / dependencies |
|---|---|---|
| LG-01 | Owned B=100/C=40 remain separate; issuer reveals only caller's issued view; observations never replace in-flight local accounting | 4,21; trusted account context |
| LG-02 | Reservation leaves100, available93; each peer independently commits93; duplicate key/token has no effect | 9,13,14; durable transactions |
| LG-03 | Requester issues7 from10, provider owns7, remaining3; next7 fails; internal use7 has no self-balance | 10,11,14; finite budgets |
| LG-04 | Exactly one concurrent60 hold from100; settle21 leaves79; explicit precommit failure releases without charge | 9,14,15,29; barriers |
| LG-05 | Reject negative/zero/overwide amounts, arithmetic overflow/underflow, final above maximum, forged tokens and unauthorized administration; exact U64_MAX spend reaches0 | 2,4,7,13,14; text boundary and typed values |
| LG-06 | Unknown trust denies paid provider work despite100 balance; bounded bootstrap still allowed; grant5 adds only5; claim/key creation does not grant trust | 7,8,26; explicit policy |
| LG-07 | Exact121 bytes and offsets1/17/49/81/113; amount9 big-endian; signature covers unsigned bytes; gross9/fee2 yields source91/recipient7 | 17–20; real Identity/Cryptography |
| LG-08 | Reject old105 layout, version/length/signature/key/caller/issuer mutations, zero, invalid/excess/mismatching fees, insufficient funds, self/issuer recipient and destination overflow; quote alone changes nothing | 17–20; real verifier, fee port |
| LG-09 | Restart exact retry returns original without charge/requote; changed order conflicts; only original/new owners query; compaction retains lifetime replay | 20,21,28; real persistence |
| LG-10 | Authenticated exact issuer evidence settles once; forwarded/wrong issuer/recipient/ID/amount/fee/reused binding never settles; refusal/loss never repeats work; disallowed policy refuses before hold/work | 12,22; trusted exchange adapter |
| LG-11 | Independent32-byte random ID/secret, portable105/version1/amount7, independent SHA256 commitment, reserved7 from20; encrypted owner-bound redemption credits7 once; secret absent from wire plaintext/logs/Board | 23–26,30; real encryption/randomness |
| LG-12 | Wrong secret/Base64/zero/issuer/owner/invalidated claim rejects; original owner retry after restart returns same amount; another owner fails; invalidation releases capacity; stored amount wins | 23–28; persistence |
| LG-13 | Exact prepare repeats same binding; changed effect conflicts; restart committed status and retry do not recharge; abort cannot undo commit; prepared timeout retains hold; SV-09 cut matrix resolves coordinator decisions | 13–16; real Ledger, test coordinator/handler |
| AT-C01 | All claimed adapters preserve exact values; ungranted callers/forged context cannot sign/spend/administer; no process pointer/path wire fields | 2,18,30; direct binding only initially |
| AT-C02 | Every configured boundary plus one outside; dependency refusal has no effects; possible commitment stays unknown until status; no automatic peer retry | 2,5,16,28 |
| AT-C03 | Immutable terms/results, owner/lifetime checks, restart obligations and replay; transient handles never become durable authority | 13–16,20,26,28 |
| AT-C04 | Truncation/cancellation never yields premature success; diagnostics exclude secrets; absent dependencies never cause hidden network/refund/trust effects | 25,30 |

## Reconciliation decisions

Pseudocode's reserve helper is a schematic summary. Normative payment modes require
requester/provider-specific local views, not writes into a remote ledger. Trust is
checked by the provider; a requester does not issue assertions about remote trust.
Common prepare/decision/status rules refine the schematic commit helper. Prepared
holds cannot be aborted merely because a deadline elapsed. Unknown payment differs
from service failure and does not authorize re-execution. No confirmed protocol
defect arises from these explicitly documented abstractions.

No standalone Ledger vector file exists at this revision. Layout fixtures must be
literal independently derived examples; real signature/envelope tests reuse the
existing cryptographic profile. Do not label local round trips as upstream vectors.

## Integration boundary

Ledger owns accounting and durable participant state, not Services' coordinator or
family databases. Linux storage is local platform persistence, independent of paid
Storage. Direct-call tests with a test coordinator cover participant semantics;
real Services SV-09 and serialized/network controls remain later integration.
Only a claimed binding can pass its acceptance rows. No minimum-node claim follows.
