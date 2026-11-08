# Services module plan

Status: implementation in progress; see checked steps and evidence below.

This is roadmap milestone 8 and planning folder 7, following completed Ledger.
Services owns typed invocation, handler dispatch and durable local coordination.
The existing Services schema-format validator remains the starting point.

## Steps

- [x] **1.** Reconcile Services requirements and acceptance cases with upstream.
- [x] **2.** Define Services authority, operation contexts and resource limits.
- [ ] **3.** Add deterministic Services dependencies and failure helpers.
- [ ] **4.** Represent the complete logical value vocabulary exactly.
- [ ] **5.** Validate structured values against bounded schemas.
- [ ] **6.** Define requester and provider handler contracts.
- [ ] **7.** Register exact contracts through authorized composition.
- [ ] **8.** Connect registered capabilities to Board selection.
- [ ] **9.** Authorize provided and wanted invocation roles.
- [ ] **10.** Assess input metadata before accepting large bodies.
- [ ] **11.** Calculate fixed prices with checked exact arithmetic.
- [ ] **12.** Validate the Computation metering exception.
- [ ] **13.** Bind quote acceptance to exact terms and context.
- [ ] **14.** Define durable coordinator records and storage outcomes.
- [ ] **15.** Implement protected local coordinator persistence.
- [ ] **16.** Reserve Ledger capacity for accepted invocations.
- [ ] **17.** Validate complete input through bounded body references.
- [ ] **18.** Journal execution attempts before dispatching work.
- [ ] **19.** Validate outputs and retain complete results durably.
- [ ] **20.** Prepare handler and Ledger effects through public contracts.
- [ ] **21.** Persist one immutable commit or abort decision.
- [ ] **22.** Deliver commit decisions without duplicate effects.
- [ ] **23.** Abort uncommitted work without erasing obligations.
- [ ] **24.** Recover coordination across every durable failure cut.
- [ ] **25.** Track payment state separately from completed work.
- [ ] **26.** Run requester-local checks without side effects.
- [ ] **27.** Coordinate paid evidence and included check entitlements.
- [ ] **28.** Drain handlers while preserving accepted recovery paths.
- [ ] **29.** Bound retention without discarding recovery obligations.
- [ ] **30.** Verify contracts through a bounded serialized adapter.
- [ ] **31.** Verify concurrent invocation and authority isolation.
- [ ] **32.** Add a discoverable local Services exercise path.
- [ ] **33.** Complete the Services acceptance matrix.
- [ ] **34.** Publish Services documentation and the Peer Protocol handoff.

## Reference baseline and scope

Reviewed on 2026-11-08 against
[`d40b6278f24430e814d7fa82094f57261bfcac9f`](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
Upstream main is six commits ahead; comparison shows website, workflow and
introductory changes, with no changes to the reviewed protocol contracts.
Recheck upstream before implementation and record the revision used for fixtures.

Sources reviewed:

- [Services specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/README.md).
- [Services inputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/inputs/README.md).
- [Services outputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/outputs/README.md).
- [Services pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/pseudocode/README.md).
- [SV-01–SV-12 acceptance cases](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/acceptanceTests/README.md).
- [Common contracts and durable coordination](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md).
- [Shared acceptance guide](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/AcceptanceTests.md).
- [Peer Protocol quote, body and check binding](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md).
- [Peer Protocol encoding vectors](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/pseudocode/TestVectors.md).
- [Computation metering contract](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/computation/README.md).

Also use the existing [Board contracts](../../src/modules/board/README.md),
[Ledger contracts and handoff](../../src/modules/ledger/README.md) and
[Services schema documentation](../../src/modules/services/schema/mod.md).
Services acceptance cases supply literal scenarios; peer encoding vectors belong
to the later Peer Protocol codec and must not be confused with a local adapter.
No new confirmed protocol defect was identified during this review. Existing
[Board findings](../../CR2SE%20bugs/README.md) remain relevant to definition and
precondition validation; verify upstream resolution rather than inventing rules.

Scope is reusable validation, authorized requester/provider dispatch, exact quotes,
real Board/Ledger integration, a durable local coordinator, protected Linux
persistence, checks and direct/serialized contract verification. Test-owned family
handlers exercise the common lifecycle without implementing paid Storage or the
optional families. Their passing scenarios establish Services behavior, not family
conformance. Unsupported family adjustments remain explicit refusals until their
family and accounting contracts are implemented.

Peer Protocol retains wire encoding, live quote/body exchanges, authenticated
network contexts and issuer controls. Production serve must continue to refuse
unsupported application protocols. Services completion alone does not complete
the minimum useful node, cross-peer atomic payment or exactly-once external work.

Keep production code under src/modules/services/ with focused schema, value,
registry, invocation, coordination, persistence and check files as responsibilities
appear; composition/examples/CLI stay outside protocol internals. Do not create
empty speculative directories. Public contracts need same-stem Markdown documents,
function explanations and safe structured entry logging under AGENTS.md.

Every implementation step includes focused unit tests, relevant integration tests,
updated contracts/docs/help and required checks before it is checked off. Measure
production coverage using [the repository procedure](../../docs/coverage.md), keep
it above 80%, and report the measured result. Commit each completed step separately
with a descriptive title and a body explaining rationale and verification, then
push according to the standing workflow. Keep evidence under the matching detail
section. Planning does not authorize execution or complete any checklist item.

## Details

### 1. Reconcile Services requirements and acceptance cases with upstream

Create an assertion-level requirements map for SV-01–SV-12, applicable AT-C01–AT-C04, Board BD-09 and Ledger LG-13. Recheck upstream revisions and existing reports before implementation. Distinguish normative lifecycle rules from schematic pseudocode, especially durable decisions and uncertainty. Record unresolved protocol questions and report confirmed language-independent defects under AGENTS.md; do not invent wire behavior.

### 2. Define Services authority, operation contexts and resource limits

Define versioned local contracts, typed failures, caller/peer identity provenance, deadlines, cancellation and finite byte/node/body/concurrency/journal limits. Separate registration, requester spending, provider invocation, observation and coordinator recovery authority. Bind attempts to requester plus connection/stream instance using the existing Ledger operation key. Test foreign, revoked, expired and incompatible contexts before effects; diagnostic text must be bounded.

### 3. Add deterministic Services dependencies and failure helpers

Provide test-owned custom handlers, immutable Board selections, controlled clocks, body sources and durable failure scripts with independently specified outputs. Reuse real Ledger fixtures where applicable. Self-test exhaustion and refusal behavior, track assess/execute/prepare/commit calls, and restart from durable bytes only. Use barriers for races and explicit boundary cuts rather than timing sleeps; helpers must never become production handlers.

### 4. Represent the complete logical value vocabulary exactly

Introduce bounded logical booleans, signed and unsigned 32/64-bit integers, binary32/binary64, UTF-8 strings, bytes, objects and arrays. Preserve float bits including signed zero and explicitly permitted nonfinite values; never route integers through floating point. Distinguish absent fields from null. Test literal SV-02 extrema and invalid representations; Peer Protocol retains ownership of canonical peer encodings.

### 5. Validate structured values against bounded schemas

Extend the existing Services schema validator without breaking Board callers. Validate required and optional fields, nested arrays, aggregate nodes/depth/bytes and service-specific restrictions through explicit semantic ports. Unknown types or required semantics are unusable; optional metadata cannot confer new behavior. Cover every SV-01–SV-03 mutation, duplicate fields and refusal before proportional allocation. Nonfinite permission must come from an understood contract, not an invented schema extension.

### 6. Define requester and provider handler contracts

Expose separate describe, assess, execute, evidence, verify and durable participant ports with owned input/result/effect references. Assessment must not execute or spend; execution may stage effects but cannot independently settle Ledger. Define exact body lengths, backpressure, explicit end/error, reference ownership and snapshot-pin release. Test substitute handlers and dependency failures through these public contracts; keep sockets and module-private tables inaccessible.

### 7. Register exact contracts through authorized composition

Register requester codecs and provider handlers independently by exact service/version. Verify whole definitions and supported pricing/check modes, reserving cr2se. for known standard contracts. A matching custom name or downloaded schema is not compatibility or execution authority. Reject conflicting registration, unsupported versions and absent implementations; do not require optional families or install code from descriptions.

### 8. Connect registered capabilities to Board selection

Implement the Board semantic capability adapter through public contracts, including required preconditions and actual provider versus requester support. Consume immutable selections and verify definition, publisher, direction and issuer. Revalidate current local offering terms at the acceptance boundary and fail changed terms explicitly. Cover BD-09 selection behavior without modifying Board private state; a schema-only registration cannot advertise an operational provider.

### 9. Authorize provided and wanted invocation roles

Derive requester/provider roles from offering direction independently of connection opener. Require an authorized local source of input and finite spending authority when a wanted publisher becomes requester. Test fabricated input, installation hints, reversed TCP direction and requester-only codecs; unsolicited offers cannot start work, load code or spend. Local same-identity work still requires explicit finite-budget authority.

### 10. Assess input metadata before accepting large bodies

Validate metadata, supported operation, preconditions, declared lengths, capacity and handler assessment before admitting body transfer or execution. Retain bounded provider snapshots and price factors and release pins on rejection, expiry or cancellation before acceptance. Test denied trust/capacity through the correct dependency boundary and all SV-04 rejection rows without successful effects.

### 11. Calculate fixed prices with checked exact arithmetic

Authorize positive fixed or precomputed prices with explicit issuer, rounding, provider-size inputs and requester ceiling. Recompute using trusted registered family rules rather than accepting arbitrary handler amounts. Reject overflow, zero prices, hidden fees and unsupported models. Preserve agreed adjustments as separately authorized data; do not implement Messaging release credits or another family algorithm as a generic refund.

### 12. Validate the Computation metering exception

Permit version-one metering only for the exact registered Computation contract, with accepted formula, duration bound and maximum. Use checked integer/rational arithmetic for maximum and final prices; test max60 and billed1201 at rate1/1000 gives2, releasing58 later. Validate billable executionError/timeLimitReached records through a contract-compatible fixture without installing a Computation runtime. Reject ordinary handlers that invent post-execution metering.

### 13. Bind quote acceptance to exact terms and context

Retain bounded quotes, exact received or emitted quote bytes/hash, offering/definition, price mode, limits, sizes, adjustments and payment binding. Accept only matching trusted stream context and explicit requester approval; reject changed hashes, issuer, contract, expiry or terms without silent reselection. Peer Protocol supplies peer serialization and quote identifiers through its future port. Test byte-preserving binding, current-Board checks and no execution before acceptance.

### 14. Define durable coordinator records and storage outcomes

Specify one authoritative writer and identity-bound accepted-attempt records containing exact terms, input identity, participant references, result retention and immutable decisions. Distinguish precommit storage failure from possible commitment. Reserve journal capacity before accepting work and gate uncertainty until recovery. Define versioning, integrity/invariant validation and explicit initialization versus open; start with a volatile test adapter and cut tests.

### 15. Implement protected local coordinator persistence

Add a Services-owned Linux persistence adapter with owner-only storage, no-follow access, exclusive writer coordination and synchronized atomic updates. Use existing public platform contracts or deliberate focused helpers, never another module's private store or paid Storage. Test real reopen, corrupt/wrong-identity data, symlinks, permissions, contention and failures before/after durable replacement. Unsupported platforms must fail explicitly.

### 16. Reserve Ledger capacity for accepted invocations

Persist the operation association and coordinate the real Ledger reservation before execution. Map provider-issued, requester-issued, internal and third-party modes to existing public Ledger terms and authority; third-party maximum plus agreed fee must fit both uint64 and the spending bound. Exact local retries reuse the binding and changed parameters conflict. Resolve lost reservation replies by original-key status/retry without a new hold; exercise finite budgets and issuer policy refusal.

### 17. Validate complete input through bounded body references

Consume accepted bodies with exact declared lengths, ordered delivery, bounded buffering and explicit completion. Run all handler-required content validation, including hashes where applicable, before successful execution admission. Test missing/extra bytes, broken source, wrong values and cancellation; incomplete input cannot become a valid result or charge. Keep local body references portable across adapters and reject raw paths or pointers as portable handles.

### 18. Journal execution attempts before dispatching work

Persist an execution-attempt marker before invoking the selected handler with immutable accepted terms and complete input. Ensure concurrent or duplicate local messages cannot execute twice. Recover staged work through handler status; never restart an external side effect after an ambiguous reply. Test explicit failure versus unknown execution, changed operation bindings and a new stream with identical input as a distinct attempt.

### 19. Validate outputs and retain complete results durably

Validate output shape, service semantics, quoted sizes, fixed/final price and effect references before successful settlement. Retain complete bounded result bytes under coordinator ownership independent of caller/session lifetime. Reject invalid output and over-bound price without exposing success. Test literal fixed7 results, valid negative business outcomes, session expiry and storage failure while retaining results; an uncertain handler effect requires recovery rather than assumed rollback.

### 20. Prepare handler and Ledger effects through public contracts

Prepare the staged handler effect and exact Ledger charge using the original key and retained validated result reference. Persist opaque owner/key/effect-bound tokens and status; duplicate prepare returns the same binding and conflicting data fails. Handle reply loss from either participant without duplicate execution or reservation. Unsupported family adjustments fail before acceptance until a compatible authorized participant exists.

### 21. Persist one immutable commit or abort decision

Allow commit only after complete valid input/result, retained result data, valid finite reservation and every required preparation. Durably record one decision before delivery and forbid replacement. Recover an absent or uncertain decision through the authoritative coordinator store, never participant timeout guesses. Test each preparation/decision ordering, conflicting decisions, failed persistence and coordinator authority isolation.

### 22. Deliver commit decisions without duplicate effects

Deliver the recorded commit using exact participant tokens and recover acknowledgements through status. Report local success only when all required local effects and result retention are durable. Gate conflicting access while a participant is unresolved and release unused metered capacity through Ledger. Test both delivery orders, duplicate delivery, one unavailable participant and exactly one resource/price effect; local completion is not proof of remote receipt.

### 23. Abort uncommitted work without erasing obligations

Record and deliver authorized abort for explicit uncommitted failures and release reservations, staged effects, pins and provisional bodies safely. Distinguish cancellation/deadlines before acceptance, during preparation and after a durable decision. Never abort committed work or infer rollback from disconnect. Test duplicate abort, commit-versus-cancel races and retained unknown status when a dependency cannot establish its outcome.

### 24. Recover coordination across every durable failure cut

Run the real Services coordinator with the real Ledger participant and a durable test-owned handler through all eight SV-09 cuts. Reopen stores using only durable state, replay original decisions/status calls and assert either consistent resource-plus-price7 or abort with neither. Verify conflicting reads/spends are gated, no execution retries occur, and lifetime service replay guards survive. Include fresh-process coordinator recovery and lost replies at both participant boundaries.

### 25. Track payment state separately from completed work

Integrate Ledger payment requests/status and authenticated original-issuer receipts through explicit dependency ports for third-party settlement. Match requester/provider/issuer/transfer identity, final gross/net and fee exactly. Preserve pending, unknown and unsettled payment without rerunning work or presenting payment refusal as service rollback. Test duplicate evidence, wrong provenance and restart; live issuer network controls remain Peer Protocol work.

### 26. Run requester-local checks without side effects

Invoke registered verifiers with original input, immutable terms, complete result and locally held expectations. Return pass/fail/inconclusive, including missing evidence/resources, and reject remote requests for local-only checks. Test that provider assertions cannot establish pass and no check implicitly spends, refunds or updates trust. Retain only authorized bounded evidence and respect independent requester codec support.

### 27. Coordinate paid evidence and included check entitlements

Bind remote evidence operations to accepted positive checkPrice in the correct issuer or a service-defined included entitlement with bounded allowance. Never create a zero-price ordinary Ledger reservation for an included check. Keep expected bytes local and send only the defined challenge. Test complete wrong evidence as paid fail, missing evidence as uncharged/inconclusive, allowance replay/exhaustion and Discovery-style included checks using explicitly test-owned fixtures.

### 28. Drain handlers while preserving accepted recovery paths

Remove disabled handlers from new selection/publication eligibility while retaining the exact implementation/contract needed by accepted and prepared work. Coordinate registration and Board publication through contracts; do not silently substitute a new version during recovery. Test disable/upgrade races, requester-only availability and restart with a required recovery handler missing, which must gate affected work explicitly.

### 29. Bound retention without discarding recovery obligations

Enforce quotas for quotes, pins, inputs, results, operations and staged bytes under one admission policy. Compact only after participant finalization and all accounting/service retry obligations permit it; conservatively retain records when no proof exists. Test full stores, interrupted cleanup, lower reopen limits and preserved lifetime replay guards. Refusal must not erase a decision, accepted result or unpaid obligation to make space.

### 30. Verify contracts through a bounded serialized adapter

Implement a local adapter that exercises the same public contracts and version/authority checks as direct calls, with exact integers, float bits, quote bytes/hash, bodies and opaque references. Keep this binding explicitly local, not a new CR2SE peer wire format. Test authenticated session association, forged callers, truncation, slow consumers, lost replies and result lifetime per SV-12. Do not claim independent-process interoperability unless a real process/channel test supplies that evidence.

### 31. Verify concurrent invocation and authority isolation

Race acceptance, reservation, execution, decision delivery, checks and handler disable using barriers and exact counters. Test untrusted descriptions, forged handles, cross-identity access, expired authority and dependency loss at effect boundaries. Capture bounded structured TRACE logs and exclude raw bodies, secret material and peer-controlled diagnostic injection. Confirm no automatic network retry, installation or trust mutation exists.

### 32. Add a discoverable local Services exercise path

Provide a runnable library example using a locally registered custom handler, real Board/Ledger contracts and explicit finite authority, clearly labeled as a local demonstration. Document initialization, running, restart/recovery and test commands from the repository root. If a CLI command is needed, implement its shared parser/help topics, every option alias and executable tests together; never advertise synthetic standard-family providers or wire Services into serve prematurely.

### 33. Complete the Services acceptance matrix

Map every SV-01–SV-12 assertion and applicable shared-contract/BD-09/LG-13 requirement to executable evidence. Run real Board/Ledger integration, Linux durable recovery and direct/serialized equivalence; label test-owned family behavior and future network/family integration separately. Use fixed independent values and applicable upstream encoding vectors without claiming local round trips prove conformance. Recheck protocol reports and record unresolved limitations honestly.

### 34. Publish Services documentation and the Peer Protocol handoff

Finish module README, same-stem public contract documents, root status/links, examples and acceptance evidence. Explain authorization, limits, decisions, result lifetime, payment uncertainty and safe recovery. Run all AGENTS.md checks, warning-free API docs, changed help/examples and production coverage above80% with the measured percentage. Mark only completed scope in this plan and roadmap, then list Peer Protocol and paid Storage integration obligations without expanding or implementing those modules.

## Planning review evidence

All 34 checklist items remain unchecked, each is one actionable sentence, and
all have matching detail sections in order. Local documentation links and
whitespace checks pass. Formatting, strict Clippy, build and overview help pass.
The full locked test suite passes with loopback sockets enabled; the sandboxed
attempt denied socket creation in three existing Transport tests. Coverage was
not rerun because this change only adds planning documentation and links.
No implementation or CLI behavior changed.

Step 1 completed: published the requirements map with SV-01–SV-12, shared contracts, BD-09 and LG-13 assertions; reconciled schematic execution with normative durable coordination. Runtime acceptance remains pending.

Step 2 completed: added separate Services authority domains, action/identity/byte scoping, revocation, contract version, operation keys, deadlines/cancellation and finite limits. Required checks pass; production coverage 8871/9245 = 95.95%.
