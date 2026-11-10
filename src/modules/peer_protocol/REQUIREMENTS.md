# Peer Protocol requirements

Baseline: [d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f), rechecked 2026-11-10. Main remains six non-protocol commits ahead. Specification, input/output contracts, pseudocode, vectors and common ownership rules were reviewed.

| Case | Required observable behavior | Plan steps |
|---|---|---|
| PP-01 | Exact record vectors and every DATA split | 4–6 |
| PP-02 | Bounded framing/JSON and stream-isolated errors | 5–8,14 |
| PP-03 | Canonical exact logical values and lossless documents | 7–10 |
| PP-04 | Descriptor traversal, Base64 and complete bodies | 10–13 |
| PP-05 | Attributed metadata and normal half-closes | 15–16 |
| PP-06 | Exact quote hash, contract and stream consent | 17–20 |
| PP-07 | Independent formulas, provider sizes and total fee bounds | 18–20,23 |
| PP-08 | Complete input/result plus markers and normal END | 21–23 |
| PP-09 | Explicit failure versus possible committed effects | 24 |
| PP-10 | Wanted roles and authorized local input | 25 |
| PP-11 | Challenge-only evidence, allowances and exact family replay | 26–28 |
| PP-12 | Authenticated Ledger controls and fee acceptance | 29–31 |
| PP-13 | Independent issuer settlement and bounded control | 32–35 |

## Integration decisions

Transport owns sockets/authentication and supplies verified ordered bytes and
normal/abnormal termination through public events. Peer Protocol must associate
opaque stream handles with a unique connection instance, never trust payload IDs,
and preserve stream isolation. Its local authority must be independent of Services
and Ledger authority. Protocol dispatch remains unregistered until usable.

Board Document preserves exact source and duplicate rejection, but its format
limits must not silently substitute for wire depth32 or wrapped response sizing.
Publication and metadata response must fit one1048576-byte control record.
Services Value and local binary codec are not peer representations. Detached
metadata requires a bounded unresolved representation before complete Value input;
existing assessment currently expects complete Value metadata, so integration
needs an explicit public metadata/body contract rather than placeholder bytes.

Services Quoter retains exact quote bytes, but peer quote IDs/encoding and
requester reservation-before-accept ordering belong to this module's adapter.
Complete received success additionally needs result.end and normal END before
Services receive. Output price/finalPrice must equal envelope price even when a
custom Requester adds no contextual validation. Never send failure as rollback
proof after local commitment. Recovery retains local journals; there is no generic
remote invocation-resume/status message.

Included checks currently count accepted attempts; pinned Peer Protocol consumes
the allowance only on complete evidence. Step27 must distinguish reservations
from consumption and preserve crash/replay guards. This is an implementation
integration defect, not evidence of a new protocol defect. Existing
[issue10](https://github.com/beothorn/CR2SE/issues/10) still limits cross-peer timeout
charging claims. Cached Messaging retries need explicit family authorization;
ordinary early results are illegal and no absent family may be synthesized.

Ledger owns immutable transfer/claim replay and fee consent; direct original
issuer evidence must be verified via a separate authenticated stream. Peer
controls cannot grant credits, create claims, change trust or acquire admin keys.

Shared AT-C01–04 evidence must cover version/authority isolation, finite admission,
exact values, dependency uncertainty, result lifetime and payload-free diagnostics.
BD-09 and LG-13 require actual module integration, not substitute-only assertions.
No runtime acceptance is claimed by this requirements map.
