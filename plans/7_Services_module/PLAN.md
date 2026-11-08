# Services module plan

Status: implementation in progress; see checked steps and evidence below.

This is roadmap milestone 8 and planning folder 7, following completed Ledger.
Services owns typed invocation, handler dispatch and durable local coordination.
The existing Services schema-format validator remains the starting point.

## Steps

- [x] **1.** Reconcile Services requirements and acceptance cases with upstream.
- [x] **2.** Define Services authority, operation contexts and resource limits.
- [x] **3.** Add deterministic Services dependencies and failure helpers.
- [x] **4.** Represent the complete logical value vocabulary exactly.
- [x] **5.** Validate structured values against bounded schemas.
- [x] **6.** Define requester and provider handler contracts.
- [x] **7.** Register exact contracts through authorized composition.
- [x] **8.** Connect registered capabilities to Board selection.
- [x] **9.** Authorize provided and wanted invocation roles.
- [x] **10.** Assess input metadata before accepting large bodies.
- [x] **11.** Calculate fixed prices with checked exact arithmetic.
- [x] **12.** Validate the Computation metering exception.
- [x] **13.** Bind quote acceptance to exact terms and context.
- [x] **14.** Define durable coordinator records and storage outcomes.
- [x] **15.** Implement protected local coordinator persistence.
- [x] **16.** Reserve Ledger capacity for accepted invocations.
- [x] **17.** Validate complete input through bounded body references.
- [x] **18.** Journal execution attempts before dispatching work.
- [x] **19.** Validate outputs and retain complete results durably.
- [x] **20.** Prepare handler and Ledger effects through public contracts.
- [x] **21.** Persist one immutable commit or abort decision.
- [x] **22.** Deliver commit decisions without duplicate effects.
- [x] **23.** Abort uncommitted work without erasing obligations.
- [x] **24.** Recover coordination across every durable failure cut.
- [x] **25.** Track payment state separately from completed work.
- [x] **26.** Run requester-local checks without side effects.
- [x] **27.** Coordinate paid evidence and included check entitlements.
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

Step 3 completed: atomic-register failure cuts, explicit clock, finite reply scripts, typed handler lifecycle counters/recovery ports and real immutable Board selection fixtures are implemented and self-tested through public contracts. Tests preserve reply-loss ambiguity and helper exhaustion. Required checks pass; production coverage 9119/9500 = 95.99%.

### 4. Represent the complete logical value vocabulary exactly

Introduce bounded logical booleans, signed and unsigned 32/64-bit integers, binary32/binary64, UTF-8 strings, bytes, objects and arrays. Preserve float bits including signed zero and explicitly permitted nonfinite values; never route integers through floating point. Distinguish absent fields from null. Test literal SV-02 extrema and invalid representations; Peer Protocol retains ownership of canonical peer encodings.

Step 4 completed: exact logical integer widths, float bits, UTF-8, bytes and nested values are implemented with independent aggregate budgets and duplicate-name rejection. Required checks pass; production line coverage with current validators is 8944/9319 = 95.98%.

### 5. Validate structured values against bounded schemas

Extend the existing Services schema validator without breaking Board callers. Validate required and optional fields, nested arrays, aggregate nodes/depth/bytes and service-specific restrictions through explicit semantic ports. Unknown types or required semantics are unusable; optional metadata cannot confer new behavior. Cover every SV-01–SV-03 mutation, duplicate fields and refusal before proportional allocation. Nonfinite permission must come from an understood contract, not an invented schema extension.

Step 5 completed: shape and registered semantic validation now compose explicitly; mutation tests cover forbidden fields, required extensions, optional omissions, wrong nested types and independent limits. Board definition tests supply missing-schema/description and binding mutations. Required checks pass; production coverage 8954/9329 = 95.98%.

### 6. Define requester and provider handler contracts

Expose separate describe, assess, execute, evidence, verify and durable participant ports with owned input/result/effect references. Assessment must not execute or spend; execution may stage effects but cannot independently settle Ledger. Define exact body lengths, backpressure, explicit end/error, reference ownership and snapshot-pin release. Test substitute handlers and dependency failures through these public contracts; keep sockets and module-private tables inaccessible.

Step 6 completed: public requester/provider ports now cover assessment, staged execution, completion recovery, exact participant decisions, checks and retained immutable blobs. Typed test substitutes verify all lifecycle paths, refusal, no execution during assessment and retained result lifetime. Required checks pass; production coverage 8965/9340 = 95.99%.

### 7. Register exact contracts through authorized composition

Register requester codecs and provider handlers independently by exact service/version. Verify whole definitions and supported pricing/check modes, reserving cr2se. for known standard contracts. A matching custom name or downloaded schema is not compatibility or execution authority. Reject conflicting registration, unsupported versions and absent implementations; do not require optional families or install code from descriptions.

Step 7 completed: authorized exact requester/provider registration, reserved-namespace catalogue checks, finite capacity and independent requester-only selection are implemented. Registry pins and draining retain accepted implementations; Board capability adapter groundwork is included. Required checks pass; production coverage 9114/9501 = 95.93%.

### 8. Connect registered capabilities to Board selection

Implement the Board semantic capability adapter through public contracts, including required preconditions and actual provider versus requester support. Consume immutable selections and verify definition, publisher, direction and issuer. Revalidate current local offering terms at the acceptance boundary and fail changed terms explicitly. Cover BD-09 selection behavior without modifying Board private state; a schema-only registration cannot advertise an operational provider.

Step 8 completed: actual Board publication/selection now uses the Services registry capabilities through public contracts; provided/wanted roles, immutable selection and stale-current-selection refusal are tested. Added reserved catalogue/capacity refusals and exact integral version interpretation. Required checks pass; production coverage 9119/9500 = 95.99%. Quote acceptance will call the same current-selection contract in step13.

### 9. Authorize provided and wanted invocation roles

Derive requester/provider roles from offering direction independently of connection opener. Require an authorized local source of input and finite spending authority when a wanted publisher becomes requester. Test fabricated input, installation hints, reversed TCP direction and requester-only codecs; unsolicited offers cannot start work, load code or spend. Local same-identity work still requires explicit finite-budget authority.

Step 9 completed: invocation authorization derives exact payer/provider roles from Board selection; wanted requester work requires local input and Request authority, provider work requires Invoke authority. Trusted peer and attempt bindings, installed contract equality, definition/input byte grants and value budgets are checked. Required checks pass; production coverage 9205/9590 = 95.99%.

### 10. Assess input metadata before accepting large bodies

Validate metadata, supported operation, preconditions, declared lengths, capacity and handler assessment before admitting body transfer or execution. Retain bounded provider snapshots and price factors and release pins on rejection, expiry or cancellation before acceptance. Test denied trust/capacity through the correct dependency boundary and all SV-04 rejection rows without successful effects.

Step 10 completed: provider assessment validates caller role, metadata budgets and authority before dispatch, bounds returned factors/metadata/pins/input capacity, then rechecks cancellation and authority. Rejection and oversized metadata have tests; no execution or reservation is performed. Required checks pass; production coverage 9259/9646 = 95.99%.

### 11. Calculate fixed prices with checked exact arithmetic

Authorize positive fixed or precomputed prices with explicit issuer, rounding, provider-size inputs and requester ceiling. Recompute using trusted registered family rules rather than accepting arbitrary handler amounts. Reject overflow, zero prices, hidden fees and unsupported models. Preserve agreed adjustments as separately authorized data; do not implement Messaging release credits or another family algorithm as a generic refund.

Step 11 completed: installed family pricing validates fixed positive terms and provider factors; explicit requester ceilings are enforced before acceptance. Fixed completions cannot introduce duration or hidden charges, and uint64 maxima remain exact. Required checks pass; production coverage 9311/9694 = 96.05%.

### 12. Validate the Computation metering exception

Permit version-one metering only for the exact registered Computation contract, with accepted formula, duration bound and maximum. Use checked integer/rational arithmetic for maximum and final prices; test max60 and billed1201 at rate1/1000 gives2, releasing58 later. Validate billable executionError/timeLimitReached records through a contract-compatible fixture without installing a Computation runtime. Reject ordinary handlers that invent post-execution metering.

Step 12 completed: only cr2se.computation version1 permits accepted metering, with exact checked rational maximum/final pricing, valid zero numerator and bounded billed duration. Literal max60/final2, uint64 overflow and ordinary-handler metering refusal tests pass. Family completion semantics and actual unused-hold release are exercised at later coordination/acceptance steps. Required checks pass; production coverage 9340/9723 = 96.06%.

### 13. Bind quote acceptance to exact terms and context

Retain bounded quotes, exact received or emitted quote bytes/hash, offering/definition, price mode, limits, sizes, adjustments and payment binding. Accept only matching trusted stream context and explicit requester approval; reject changed hashes, issuer, contract, expiry or terms without silent reselection. Peer Protocol supplies peer serialization and quote identifiers through its future port. Test byte-preserving binding, current-Board checks and no execution before acceptance.

Step 13 completed: finite transient quotes retain exact bytes/SHA256 and are bound to original peer, requester/connection/stream and owner domain. Explicit acceptance rechecks current complete Board terms, price ceiling, expiry and authority; wrong hashes, streams, foreign brokers, duplicate acceptance, capacity and removal are tested. Disconnect releases only unaccepted state. Required checks pass; production coverage 9482/9881 = 95.96%.

### 14. Define durable coordinator records and storage outcomes

Specify one authoritative writer and identity-bound accepted-attempt records containing exact terms, input identity, participant references, result retention and immutable decisions. Distinguish precommit storage failure from possible commitment. Reserve journal capacity before accepting work and gate uncertainty until recovery. Define versioning, integrity/invariant validation and explicit initialization versus open; start with a volatile test adapter and cut tests.

Step 14 progress: exclusive atomic complete-image persistence, identity/authority binding, compare-and-replace, known-failure versus uncertainty gating and reload recovery are implemented. A bounded exact local value codec provides versionable image primitives. Tests cover lost acknowledgements, immutable copies, conflicts and malformed encodings. The concrete invocation-record schema is now integrated and verified with durable runtime admission, including exact retained terms, inputs/results, lifecycle prerequisites and tokens. Step14 is complete. Required checks pass; production coverage 9749/10161 = 95.95%.

### 15. Implement protected local coordinator persistence

Add a Services-owned Linux persistence adapter with owner-only storage, no-follow access, exclusive writer coordination and synchronized atomic updates. Use existing public platform contracts or deliberate focused helpers, never another module's private store or paid Storage. Test real reopen, corrupt/wrong-identity data, symlinks, permissions, contention and failures before/after durable replacement. Unsupported platforms must fail explicitly.

Step 15 completed: Services-owned Linux store now validates owner-only anchored directories/files, rejects symlinks, retains exclusive writer locking and acknowledges synchronized atomic snapshots. Framing/version/size checks, reopen, contention and all five write/sync cuts pass. This verifies simulated filesystem cuts, not physical power loss. Required checks pass; production coverage 9917/10340 = 95.91%.

### 16. Reserve Ledger capacity for accepted invocations

Persist the operation association and coordinate the real Ledger reservation before execution. Map provider-issued, requester-issued, internal and third-party modes to existing public Ledger terms and authority; third-party maximum plus agreed fee must fit both uint64 and the spending bound. Exact local retries reuse the binding and changed parameters conflict. Resolve lost reservation replies by original-key status/retry without a new hold; exercise finite budgets and issuer policy refusal.

Step 16 completed: durable admission retains exact quote/Board/definition/publisher/direction, roles, pricing, metadata, factors and input bound before one real Ledger reservation. Original-key retries retain one price7 hold (100 balance,93 available); changed bytes conflict, fee ceilings are checked and foreign implementation pins are refused. Concrete private record/version/phase validation now backs the journal. Required checks pass; production coverage 10301/10881 = 94.67%.

### 17. Validate complete input through bounded body references

Consume accepted bodies with exact declared lengths, ordered delivery, bounded buffering and explicit completion. Run all handler-required content validation, including hashes where applicable, before successful execution admission. Test missing/extra bytes, broken source, wrong values and cancellation; incomplete input cannot become a valid result or charge. Keep local body references portable across adapters and reject raw paths or pointers as portable handles.

Step 17 completed: bounded ordered bodies reject gaps, overlap, missing/excess bytes and hash mismatch before producing immutable complete bytes. Runtime input admission checks original byte limits, schema and installed content/metadata validation, persists owned input before execution and rejects changed retries. Independent SHA256(abc) and real Ledger no-charge/unchanged-balance scenarios pass; foreign implementation admission is explicitly tested. Required checks pass; production coverage 10393/10975 = 94.70%.

### 18. Journal execution attempts before dispatching work

Persist an execution-attempt marker before invoking the selected handler with immutable accepted terms and complete input. Ensure concurrent or duplicate local messages cannot execute twice. Recover staged work through handler status; never restart an external side effect after an ambiguous reply. Test explicit failure versus unknown execution, changed operation bindings and a new stream with identical input as a distinct attempt.

Step 18 completed: execution requires reserved complete input and persists an Executing marker before dispatch. Retried starts in Executing return Unknown without running again. Handlers now receive explicit public offering/definition, quote, factors and price snapshots rather than needing coordinator-private decoding. Real Board/Ledger integration verifies lifecycle and preserved uncharged holds. Required checks pass; production coverage 10464/11047 = 94.72%.

### 19. Validate outputs and retain complete results durably

Validate output shape, service semantics, quoted sizes, fixed/final price and effect references before successful settlement. Retain complete bounded result bytes under coordinator ownership independent of caller/session lifetime. Reject invalid output and over-bound price without exposing success. Test literal fixed7 results, valid negative business outcomes, session expiry and storage failure while retaining results; an uncertain handler effect requires recovery rather than assumed rollback.

Step 19 completed: staged completion recovery independently validates output schema/semantics, contextual quote rules, complete retained bytes/effect and recalculated final price before durable staging. Explicit invalid-output and price8-versus-quote7 fixtures remain uncharged and cannot trigger execution again. Results are coordinator-owned rather than session-owned. Required checks pass; production coverage 10517/11110 = 94.66%.

### 20. Prepare handler and Ledger effects through public contracts

Prepare the staged handler effect and exact Ledger charge using the original key and retained validated result reference. Persist opaque owner/key/effect-bound tokens and status; duplicate prepare returns the same binding and conflicting data fails. Handle reply loss from either participant without duplicate execution or reservation. Unsupported family adjustments fail before acceptance until a compatible authorized participant exists.

Step 20 completed: pinned family preparation returns an owner/key/effect token that is journaled before real Ledger preparation. Ledger references bind exact retained input/output/result/effect and terms; repeated preparation preserves one hold and token association without execution. Fixture tokens now explicitly bind original keys. Required checks pass; production coverage 10584/11165 = 94.80%.

### 21. Persist one immutable commit or abort decision

Allow commit only after complete valid input/result, retained result data, valid finite reservation and every required preparation. Durably record one decision before delivery and forbid replacement. Recover an absent or uncertain decision through the authoritative coordinator store, never participant timeout guesses. Test each preparation/decision ordering, conflicting decisions, failed persistence and coordinator authority isolation.

Step 21 completed: immutable decision recording permits commit only after complete retained result and both preparations. Exact decision replay is idempotent; premature commits and commit-to-abort replacement conflict before delivery. Tests retain the pending Commit state separately from terminal success. Required checks pass; production coverage 10603/11186 = 94.79%.

### 22. Deliver commit decisions without duplicate effects

Deliver the recorded commit using exact participant tokens and recover acknowledgements through status. Report local success only when all required local effects and result retention are durable. Gate conflicting access while a participant is unresolved and release unused metered capacity through Ledger. Test both delivery orders, duplicate delivery, one unavailable participant and exactly one resource/price effect; local completion is not proof of remote receipt.

Step 22 completed: commit delivery queries original participant status, skips committed effects, delivers only matching preparations and acknowledges terminal success only after both durable statuses. New admission/execution gates incomplete decisions; retained results remain Unknown before completion. Real Ledger credit100 becomes93 once across duplicate delivery, with exact retained output/price7. Required checks pass; production coverage 10680/11252 = 94.92%.

### 23. Abort uncommitted work without erasing obligations

Record and deliver authorized abort for explicit uncommitted failures and release reservations, staged effects, pins and provisional bodies safely. Distinguish cancellation/deadlines before acceptance, during preparation and after a durable decision. Never abort committed work or infer rollback from disconnect. Test duplicate abort, commit-versus-cancel races and retained unknown status when a dependency cannot establish its outcome.

Step 23 completed: only stored Abort decisions release original handler/Ledger preparations; committed work cannot be undone. Cancellation alone retains holds and decisions. Tests cover invalid-result abort, idempotent release and an admission failure with no Ledger hold, proving abort never creates a replacement reservation. Required checks pass; production coverage 10732/11289 = 95.07%.

### 24. Recover coordination across every durable failure cut

Run the real Services coordinator with the real Ledger participant and a durable test-owned handler through all eight SV-09 cuts. Reopen stores using only durable state, replay original decisions/status calls and assert either consistent resource-plus-price7 or abort with neither. Verify conflicting reads/spends are gated, no execution retries occur, and lifetime service replay guards survive. Include fresh-process coordinator recovery and lost replies at both participant boundaries.

Step 24 completed: the real coordinator, real Ledger and byte-persistent test handler survive all eight SV-09 cuts with fresh objects/authority domains, exact decision replay and no re-execution. Known uncommitted cases resolve through an explicit recovered-coordinator abort; durable commits finish resource+price7 once. A separately exec'd Linux test reopens protected stores and confirms durable result/accounting. Added bounded resume of original phases without execution. Required checks pass; production coverage 10789/11314 = 95.36% (valid LLVM profiles; existing abrupt-child profile warning handled by the documented procedure).

### 25. Track payment state separately from completed work

Integrate Ledger payment requests/status and authenticated original-issuer receipts through explicit dependency ports for third-party settlement. Match requester/provider/issuer/transfer identity, final gross/net and fee exactly. Preserve pending, unknown and unsettled payment without rerunning work or presenting payment refusal as service rollback. Test duplicate evidence, wrong provenance and restart; live issuer network controls remain Peer Protocol work.

Step 25 completed: Services exposes original Ledger payment observations, explicit issuer actions, authenticated receipt confirmation and independent refusal/uncertainty updates. Real third-party provider tests keep valid work committed while payment transitions Pending/Unsettled/Unknown/Settled; gross9/fee2 credits7 once and wrong gross is denied without re-execution. Required checks pass; production coverage 11072/11643 = 95.10%.

### 26. Run requester-local checks without side effects

Invoke registered verifiers with original input, immutable terms, complete result and locally held expectations. Return pass/fail/inconclusive, including missing evidence/resources, and reject remote requests for local-only checks. Test that provider assertions cannot establish pass and no check implicitly spends, refunds or updates trust. Retain only authorized bounded evidence and respect independent requester codec support.

Step 26 completed: local verification uses retained original input/terms/output and privately borrowed expectations. Pass/fail/inconclusive are requester-computed; no provider call, spending, refund or trust update occurs. Real Ledger balances/trust remain unchanged across all outcomes. Missing successful evidence or unavailable verification is inconclusive. Required checks pass; production coverage 11111/11687 = 95.07%.

### 27. Coordinate paid evidence and included check entitlements

Bind remote evidence operations to accepted positive checkPrice in the correct issuer or a service-defined included entitlement with bounded allowance. Never create a zero-price ordinary Ledger reservation for an included check. Keep expected bytes local and send only the defined challenge. Test complete wrong evidence as paid fail, missing evidence as uncharged/inconclusive, allowance replay/exhaustion and Discovery-style included checks using explicitly test-owned fixtures.

Step 27 completed: paid evidence and included entitlements share the durable invocation namespace, with distinct challenge/evidence contracts, private expectations, current checkPrice/issuer and finite stable-scope allowances. Complete wrong evidence is paid fail; malformed or timed-out requester evidence is uncharged/inconclusive; included checks create no Ledger operation. Restored timing is clock-domain-aware and never reruns evidence reads. Private payload schema2 records purpose/allowance/timing. Required checks pass; production coverage 11640/12335 = 94.37%. Confirmed the postcommit two-node timeout ambiguity and filed [CR2SE#10](https://github.com/beothorn/CR2SE/issues/10), with [local evidence](../../CR2SE%20bugs/0007-check-timeout-settlement.md); no global uncharged-timeout claim is made.

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

Requester-path completion audit (SV-07/SV-11): added independently validated remote
assessments, requester-only wanted dispatch and original-peer complete-result/failure
events. No local provider installation or automatic peer retry occurs. Unknown
remote outcomes retain holds; valid received results settle finite local issuance.
Required checks pass; production coverage 10978/11513 = 95.35%.
