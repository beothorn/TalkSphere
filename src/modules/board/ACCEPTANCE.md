# Board acceptance inventory

Behavior baseline: [CR2SE d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
Rechecked on 2026-11-06: upstream main has six website/documentation commits
beyond this revision and no protocol changes. No Board implementation assertions
have run yet; this inventory maps requirements to future executable evidence.

## Required suite

Source: [BD-01–BD-10](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/acceptanceTests/README.md).
All entries below are pending unless explicitly updated with a test reference.

| Case | Assertions to execute | Plan steps |
|---|---|---|
| BD-01 | Empty/populated documents; exact matching fields; schemas only in definitions | 5–9, 12–13 |
| BD-02 | Missing/wrong/unsupported fields; UTF-8; malformed/trailing JSON; duplicate members; unchanged state; exact 9007199254740993 and U64_MAX re-emission | 4–6, 12–13 |
| BD-03 | Every required field mutation; duplicate IDs in either/across arrays; no first/last wins; distinct IDs sharing version | 5, 9, 12 |
| BD-04 | Prices 1/U64_MAX; every invalid numeric/type row; exclusive pricing; checkPrice; forbidden embedded schemas; model maximum overflow | 6, 10 |
| BD-05 | Provided/wanted roles independent of TCP opener; exact issuer; rejected aliases and invalid IDs | 5, 19 |
| BD-06 | Known offering survives unknown service/version/model; unknown preconditions block use; nested unknown metadata preserved; hints produce no privileged effects | 4, 6–10, 19, 22 |
| BD-07 | Each definition identity mismatch; remote mismatch unusable; missing/removed definition never substituted; old snapshot preserved | 9, 12–13, 18 |
| BD-08 | Atomic reader/publisher races; immutable pins; failure/restart retains acknowledged complete snapshot | 13–16, 22 |
| BD-09 | Changed future selections and retained copied terms | 19–20 |
| BD-09 integration | Actual quote price change needs explicit acceptance; accepted work survives removal with original obligations | Deferred to Services/Peer Protocol; test-owned consumer is only Board evidence |
| BD-10 | Direct authenticated provenance vs forwarded content; wrapped response size; array order has no implicit priority | 11, 17, 19 |

## Shared cases

Source: [AT-C01–AT-C04](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/AcceptanceTests.md#common-contract-cases).

| Case | Applicable assertions | Plan steps |
|---|---|---|
| AT-C01 | Declared direct ports, exact values, caller authority, no payload-forged provenance, no pointer/path peer fields | 2, 12–19, 22–23 |
| AT-C02 | Every configured boundary; fail before effects; dependency failure vs uncertain commit; no automatic external retry | 2–4, 10–12, 14–18 |
| AT-C03 | Immutable pinned results; owner/lifetime isolation; acknowledged state survives restart | 13–20, 22 |
| AT-C04 | No partial successful document; malformed/truncated/cancelled input; secret-free bounded diagnostics; absent dependencies cause no hidden calls | 4, 12, 16–18, 21–22 |

Serialized local binding is not supplied by this plan; no process-separation claim.
Board has no spending, refund or provider check-outcome port; those assertions
belong to Services/Ledger and are inapplicable to Board alone.

## Interpretation and fixtures

The normative specification, input/output contracts, Services schema vocabulary,
Peer Protocol metadata limits and shared module contracts are linked in the
[plan](../../../plans/5_Board_module/PLAN.md). No standalone Board vector file is
published at the baseline. New literal examples are specification-derived fixtures,
not upstream cryptographic vectors. Use Identity-derived valid IDs, complete
matching definitions and independent literal numeric expectations.

Strict rejection of malformed Boards is TalkSphere policy. Structurally valid
unknown semantics remain discoverable but cannot authorize use. Publication
capabilities distinguish provided handlers from wanted requester codecs/input.
Cache lifetime and limits are local policy; snapshots never introduce a wire
revision field. Full wrapper JSON must fit 1,048,576 bytes and depth 32.

Unresolved reports [#8](https://github.com/beothorn/CR2SE/issues/8) and
[#9](https://github.com/beothorn/CR2SE/issues/9) contradict normative duplicate-member
and precondition rules. Follow mandatory rejection and nonempty namespaced names;
do not invent namespace grammar before upstream clarification. Existing local
reports remain reported upstream; no fix has been verified.
