# Services requirements and evidence map

Baseline: [d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f), reviewed 2026-11-08.
The [specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/README.md), input/output contracts and
[common recovery rules](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md)
are authoritative over simplified execution pseudocode.

This is an implementation map, not passing acceptance evidence. Existing schema
format tests cover only structural descriptions. All runtime rows remain pending.

| Case | Required assertions | Planned steps / dependencies |
|---|---|---|
| SV-01 | Complete input/output/check; nonempty nested descriptions; unknown types and mismatching versions rejected; names alone do not establish compatibility | 5–8; existing Board definitions |
| SV-02 | Exact signed/unsigned extrema; bool/text/bytes; float bits and signed zero; explicit nonfinite permission; invalid types/ranges refused | 4–5 |
| SV-03 | Required/optional fields, absence versus null, arrays, unknown optional fields, bounded depth/count/bytes, discriminator restrictions | 5; family semantic validation |
| SV-04 | Metadata before body; acceptance then one reservation then execution; immutable terms; unknown precondition, handler, issuer, price, definition, trust and capacity refusals have no effects | 8–16; Board/Ledger |
| SV-05 | Missing/extra body bytes, wrong value/content hash cannot charge; valid fixed7 charges once; invalid output/price8 cannot succeed | 17–22; handler validation |
| SV-06 | Computation max60/final2 releases58; valid error/limit completion is billable; empty pages are successes; ordinary metering refused | 11–12,19,22; registered family rules |
| SV-07 | Wanted input and payer require local authority; hints cannot install/run/spend; TCP direction does not choose roles | 9 |
| SV-08 | Expected byte stays local; wrong complete evidence is paid fail; missing evidence uncharged/inconclusive; included entitlement bounded; local-only checks have no remote call; missing expectations inconclusive | 26–27 |
| SV-09 | Eight durable cuts; restart from bytes; immutable decision; at most one visible resource and price7; no timeout abort, re-execution, duplicate hold or partial success; conflicting work gated; replay guards retained | 14–24,29; real Ledger and durable test handler |
| SV-10 | Explicit uncommitted failure releases hold; lost reply is unknown; cancellation preserves committed obligation; new stream is new attempt; no automatic retry | 18,23–24 |
| SV-11 | Optional handler absent before acceptance; requester-only wanted path works; disable preserves accepted recovery | 7–9,28 |
| SV-12 | Direct/serialized identity/value/quote/body equivalence; durable result survives session expiry; unknown version and forged commit denied | 19,30–31 |
| AT-C01 | Dependencies substitutable; no private state access; foreign/revoked caller rejected | 2–3,6,30–31 |
| AT-C02 | Finite admission, exact values, dependency refusal/timeout/unknown preserved | 2,4–5,14–24,29 |
| AT-C03 | Immutable selection/result ownership and durable replay-safe state | 8,13–24,28–30 |
| AT-C04 | Explicit supported outputs, bounded secret-free diagnostics, no optional dependency activation | 7,25–27,31–34 |
| BD-09 | Board selection and quoted terms agree; changed terms require explicit acceptance | 8,13,33 |
| LG-13 | Real coordinator delivers immutable participant decisions after loss/restart without double spending | 16,20–24,33 |

## Reconciled interpretation

The Services execution pseudocode's atomic commit/release shorthand denotes the
common prepare/decision/delivery protocol when stores are independent. It does
not authorize releasing a reservation after an uncertain commit. A durable
execution marker precedes external effects; recovery retries decisions, never
execution. Generic identical requests on fresh streams are separate attempts.

Validation of logical values belongs here; canonical peer JSON and detached-body
records belong to Peer Protocol. A local serialized adapter is a separate local
binding and must preserve authority provenance. No schema extension for nonfinite
permission is invented: registered contract semantics must supply that permission.

Family-specific prices, output rules, adjustments and replay guards remain family
responsibilities. Unsupported adjustments must fail before acceptance. Test
families prove common lifecycle behavior without advertising Storage, Computation
or other standard-family support. No new confirmed protocol defect was found;
existing Board issues #8/#9 remain linked in the local bug index.

## Validation

Run the repository checks and `cargo test --locked modules::services` from the
repository root. Replace pending rows with assertion-level evidence as the plan
progresses; do not infer conformance from a round trip or scaffolding.
