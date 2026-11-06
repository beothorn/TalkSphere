# Board module plan

Status: all 24 steps complete; deferred Services/Peer Protocol integration is documented.

This is roadmap milestone 6 and planning folder 5, following completed
Transport integration. Board owns consistent offering/definition snapshots and
attributed caches; it uses the Services schema format without its runtime.

## Steps

- [x] **1.** Reconcile Board requirements and acceptance cases with upstream.
- [x] **2.** Define Board contracts, authority and resource limits.
- [x] **3.** Add deterministic Board fixtures and fault-injection helpers.
- [x] **4.** Decode bounded JSON without losing numbers or unknown metadata.
- [x] **5.** Validate Board headers and offering identity fields.
- [x] **6.** Validate exact prices and optional offering metadata.
- [x] **7.** Validate primitive service schema descriptions.
- [x] **8.** Validate nested service schemas and check definitions.
- [x] **9.** Match complete definitions to their offerings.
- [x] **10.** Assess supported service semantics through explicit capability ports.
- [x] **11.** Check publication sizes against complete peer control responses.
- [x] **12.** Prepare complete publication candidates without changing visible state.
- [x] **13.** Publish immutable Board snapshots atomically in memory.
- [x] **14.** Define durable snapshot commits and recovery outcomes.
- [x] **15.** Implement protected local snapshot persistence.
- [x] **16.** Verify publication recovery across failures and restarts.
- [x] **17.** Cache remote Boards with explicit provenance and bounded lifetimes.
- [x] **18.** Associate remote definitions with the selected cached snapshot.
- [x] **19.** Select usable offerings with explicit roles and issuer.
- [x] **20.** Invalidate stale selections without changing retained terms.
- [x] **21.** Expose authorized local Board validation and inspection commands.
- [x] **22.** Verify concurrent access, authority isolation and diagnostic privacy.
- [x] **23.** Complete the Board acceptance matrix through public contracts.
- [x] **24.** Publish Board documentation and the next-module handoff.

## Reference baseline and scope

Reviewed on 2026-11-06 against
[`d40b6278f24430e814d7fa82094f57261bfcac9f`](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
Comparison with upstream `main` found six later commits affecting the website,
workflow and introductory material, with no changes to the reviewed contracts.
Recheck upstream at implementation time and record any newly adopted revision.

Sources reviewed:

- [Board specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/README.md), [inputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/inputs/README.md), [outputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/outputs/README.md), [pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/pseudocode/README.md) and [BD-01–BD-10](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/acceptanceTests/README.md).
- [Services definitions and type vocabulary](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/README.md#separately-retrievable-definition).
- [Peer Protocol records, limits and metadata exchanges](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md).
- [Module contracts](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md) and [shared acceptance guide](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/AcceptanceTests.md).

Board has acceptance pseudocode and examples; no standalone
`modules/board/pseudocode/TestVectors.md` exists at this revision. Build literal
fixtures from the pinned specifications, replacing placeholder IDs with valid
Identity values, and label them as derived fixtures rather than upstream vectors.

Confirmed planning findings are [0005](../../CR2SE%20bugs/0005-board-duplicate-json-members.md)
([upstream #8](https://github.com/beothorn/CR2SE/issues/8)) and
[0006](../../CR2SE%20bugs/0006-board-precondition-validation.md)
([upstream #9](https://github.com/beothorn/CR2SE/issues/9)). Follow normative duplicate
rejection and nonempty precondition rules; do not copy the contradictory
pseudocode. Recheck namespace clarification before choosing validation syntax.
These are protocol reports, not evidence of implemented regression tests.

Keep Board code under `src/modules/board/`, organized into focused document,
validation, snapshot, cache, selection and persistence responsibilities as each
becomes useful. Keep the reusable Services schema-format validator under
`src/modules/services/schema/` with its own format-only documentation; this does
not implement or complete Services. Reuse Identity's public textual-ID parser.
Composition owns platform wiring and trusted capability grants; CLI stays in
`src/cli/`. Do not reach into Identity's private storage or Transport state.

Implement direct-call Board contracts with a Linux durable adapter and injected
platform substitutes. Peer Protocol, service execution, quotes, Ledger spending,
standard-family implementations and serialized local adapters remain later work.
Board selection cannot authorize execution or payment. Production must not
register `cr2se.peer` or advertise absent providers. A format-valid custom document
is not proof that a handler exists. Wanted publication requires requester-side
capabilities and authorized input, rather than a local provider.

## Execution rules

Follow [AGENTS.md](../../AGENTS.md) on every implementation step: add unit and
applicable integration/failure tests, source explanations, entry tracing and
same-stem public contract documents; update build/run instructions and help when
observable behavior changes. Run required formatting, Clippy, locked tests/build
and help checks, exercise changed commands and measure production coverage above
80% using [the coverage procedure](../../docs/coverage.md). Commit each completed
step separately with a descriptive title and body recording purpose and checks;
update its checklist and evidence only after those checks pass. Follow standing
GitHub reporting instructions for newly confirmed protocol defects.

## Details

### 1. Requirements and acceptance inventory

Create the module acceptance matrix linking each BD-01–BD-10 row and applicable
AT-C01–AT-C04 assertion to planned tests. Re-read the pinned sources and upstream
fixes, record fixture provenance, and distinguish normative rules from illustrative
pseudocode and local policy. Resolve or explicitly track contradictions rather
than silently choosing wire behavior. Record which BD-09 quote/acceptance assertions
require later Services/Peer Protocol integration; Board tests can prove immutable
terms and staleness only. Preserve the completed integration evidence.

Completed: re-read current upstream Board specification, contracts, pseudocode and
acceptance suite; protocol baseline is unchanged. Added the assertion-level
[acceptance inventory](../../src/modules/board/ACCEPTANCE.md), including deferred
BD-09 integration, fixture provenance and unresolved upstream reports.

### 2. Contracts, authority and limits

Define publication, read, definition, cache and selection ports with typed errors,
opaque immutable snapshot references, publisher identity and contract version.
Specify owner/lifetime rules, cancellation/deadlines and trusted local grants;
peer document fields never supply administrative authority. Choose finite byte,
depth, node, string, array, definition-count, cache and retained-snapshot budgets.
Document strict rejection of malformed Boards as local policy, while keeping
structurally valid unknown semantics discoverable. Test invalid configuration,
unauthorized operations and scaffold failure without claiming module completion.

Completed: added explicit typed boundary errors, finite validated limits, identity/action/
byte-scoped revocable grants, deadline/cancellation checks, immutable snapshot
contract and logical publication/definition/cache/selection ports. Scaffold reads
return Unsupported. Focused unit tests and all repository checks pass; production
coverage is 5484/5685 = 96.46%.

### 3. Deterministic dependencies and fixtures

Provide test-owned persistence with commit/failure cut points, clock, authority,
capability/semantic registries and controlled concurrency barriers. Use complete
literal offered/wanted definitions and valid identity text; independent expected
numbers include 9007199254740993 and uint64 maximum. Helpers must observe effects,
not reproduce production validation. Cover helper failures and cleanup; keep
substitutes test-only and exclude them from production coverage.

Completed: added test-only complete literal documents with upstream-derived issuer,
independent exact prices, controlled time/capabilities and atomic commit cut-point
observations preserving durable state across restart. Helper self-tests cover
failures and script exhaustion. Reuse standard barriers/channels for future races
and the existing process harness. All required checks pass; production coverage
remains 5484/5685 = 96.46%, excluding test support.

### 4. Lossless bounded JSON

Evaluate a deliberate JSON dependency or bounded decoder that retains exact number
values/tokens, validates UTF-8 and rejects malformed/trailing input and duplicate
members at every depth, including equivalent escaped names. Enforce limits during
parsing before excessive allocation. Preserve nested unknown fields and exact
numbers when relaying originals; retaining source bytes is acceptable and avoids
unrequested normalization. Test numeric extremes, fractions/exponents without
lossy coercion, invalid Unicode, deep nesting and every configured boundary.
Do not convert Board uint64 prices to Peer Protocol decimal-string value encoding.

Completed: Added bounded structural preflight, decoded duplicate-key rejection and exact source preservation with arbitrary-precision numeric trees; tested malformed input and resource boundaries. Required checks pass. Production line coverage: 5601/5805 = 96.49%

### 5. Board and offering identities

Require version 1 and both arrays, including valid empty lists; the version is
not a publication counter. Validate nonempty id/service/description, exact
serviceVersion 1 through uint32 maximum and explicit issuer through Identity.
Reject duplicate offering IDs within and across lists without first/last wins.
Keep direction explicit and unknown reserved-prefix services unsupported rather
than automatically malformed. Test same service/version under distinct IDs,
unsupported versions, issuer aliases, nulls, wrong types and boundary values.

Completed: Added Board version/list validation, strict issuer parsing, direction-aware offerings and duplicate rejection across lists with malformed-field unit tests. Required checks pass. Production line coverage: 5652/5857 = 96.50%

### 6. Prices and metadata

Enforce exactly one of fixed price or pricing object, positive uint64 price and
checkPrice, nonempty pricing.model, object-valued info and no embedded schemas.
Validate preconditions and suggestions according to normative rules and the
tracked clarification; empty optional suggestion strings remain distinct from
empty required names. Preserve unknown metadata without executing descriptions,
URIs or hints. Test overflow, both/neither price forms, wrong optional types,
unknown required semantics and inert install/run/spend text.

Completed: Enforced exclusive pricing, positive uint64 amounts, optional metadata types and embedded-schema rejection. Empty preconditions are rejected; unresolved namespace grammar remains tracked under upstream issue 9 without invented syntax. Required checks pass. Production line coverage: 5700/5908 = 96.48%

### 7. Primitive schema descriptions

Add the reusable format-only validator for bool, signed/unsigned integer widths,
float/double, string and bytes descriptors. Require a type field and preserve
unknown optional metadata. Represent unknown types as unsupported semantics;
never substitute a known type or infer executable rules from descriptions.
Document that this validates schema documents, not invocation values, detached
bodies or runtime float serialization. Test malformed descriptors, unknown types
and exact preservation without creating a Services runtime dependency.

Completed: Added format-only primitive schema vocabulary with explicit unknown semantic support and malformed-descriptor tests; no Services runtime capability is advertised. Required checks pass. Production line coverage: 5710/5918 = 96.49%

### 8. Composite schemas and checks

Validate object fields recursively, array items, required-by-default fields,
optional:true only for object fields, and nonempty descriptions for every named
field including nested fields. Validate check.description plus check.input/output;
no implicit nullability or new semantics from metadata. Bound recursive work and
account for nested unsupported types. Test missing/wrong fields, invalid optional
placement, malformed checks and depth/node limits with complete valid controls.

Completed: Added bounded recursive object and array schemas, named-field descriptions, field-only optionality and mandatory check schemas; tested malformed and unknown nested types. A transient truncated LLVM profile required rerunning coverage. Required checks pass. Production line coverage: 5745/5955 = 96.47%

### 9. Matching definitions

Require complete id/service/serviceVersion/description/input/output/check and
match the four identity fields exactly to the offering. Reject economic terms
in definitions and fail missing or removed IDs rather than substituting versions.
Keep custom logical-contract comparison explicit: names alone do not establish
compatibility, and schema shape cannot redefine a standard service. Test each
mismatch independently and contradictory same-service/version definitions using
a declared contract validator, without treating different offering IDs as errors.

Completed: Added complete definition parsing, four-field matching, economic-field rejection and conservative full-contract comparison; tested every missing and mismatching field and unknown schema support. Required checks pass. Production line coverage: 5779/5989 = 96.49%

### 10. Capability and semantic assessment

Introduce narrow ports for service/version support, schema support, pricing-term
validation and required preconditions. Distinguish structural validity, local
usability and eligibility for local publication. Validate understood family terms
and advertised maxima with checked exact arithmetic; test overflow through an
injected model without claiming real Storage pricing. Unknown models/types block
use of only that offering. Enforce supplied capabilities differently for provided
and wanted roles, and check cr2se.identity from authenticated context rather than
a second handshake. No hidden network calls, spending or runtime handler stubs.

Completed: Added trusted exact-contract, term-validation, precondition and direction-specific local capability ports; unknown semantics remain unusable and known arithmetic failures propagate without hidden effects. Required checks pass. Production line coverage: 5815/6025 = 96.51%

### 11. Retrievable document bounds

Measure the exact encoding of {"type":"board","board":...} and
{"type":"definition","definition":...}, including escaping and wrapper overhead,
against the 1,048,576-byte control-body maximum and depth 32. Do not apply the
five-byte record header to the body budget or allow a standalone document to
consume the entire wrapped budget. Test exact fit/one over, nested wrapper depth,
large Unicode strings and unknown metadata. Keep response sizing a small shared
format boundary for later Peer Protocol reuse, not a partial network exchange.

Completed: Added exact wrapped response sizing with original bytes, wrapper overhead, fixed control-body maximum and reserved nesting depth; tested exact fit and one-byte overflow for both response kinds. Required checks pass. Production line coverage: 5835/6045 = 96.53%

### 12. Publication preparation

Validate the complete candidate, all matching definitions, logical-contract
consistency, semantic/capability eligibility and retrievability before effects.
Specify treatment of unreferenced definitions as local policy and bound aggregate
candidate work. Return an immutable prepared candidate whose validation cannot
be bypassed by callers; recheck relevant authority/capability changes at commit.
Test every rejection preserves the prior snapshot and produces no persistence
write, provider registration or other external action.

Completed: Validated complete Board/definition sets, aggregate budgets, peer retrievability, contract consistency and current local capabilities before any effects; rejected missing and extra definitions. Required checks pass. Production line coverage: 5887/6106 = 96.41%

### 13. Atomic in-memory publication

Replace one complete Board/definition snapshot with an owner-issued reference;
readers pin a consistent revision and removed definitions fail in current reads.
Serialize concurrent publishers and prevent lost updates according to a documented
local policy. Bound retained pins without mutating already returned content.
Race readers and publishers with barriers in both orders and verify complete
old/new pairs, immutable prior reads, empty replacements and released resources.
No new revision field is added to peer JSON.

Completed: Added serialized complete revision replacement, owner-bound immutable pins, exact pinned-definition reads and finite retained-revision admission. Revalidate candidates under receiving limits before visibility. Required checks pass. Production line coverage: 5985/6217 = 96.27%

### 14. Durable commit contract

Define platform persistence for one validated snapshot, including publisher
binding, format version, serialization order, startup recovery and the exact
acknowledgement point. Separate preparation failure from a possibly committed
write with lost acknowledgement; uncertain outcomes require recovery/status,
not assumed rollback or automatic republication. Test this contract with injected
cuts and define when reads/new writes must wait for reconciliation. Private
operation identifiers are local metadata, not new protocol fields.

Completed: Added complete durable records and a persistence port; commit precedes visibility, uncertain outcomes gate new state, and explicit recovery revalidates stored content without replaying writes. Required checks pass. Production line coverage: 6018/6309 = 95.39%

### 15. Linux persistence adapter

Implement a bounded, protected snapshot store with exclusive writer coordination,
atomic replacement and required file/directory synchronization before success.
Keep public Board data separate from Identity's secrets and internal store format;
validate publisher ownership, permissions, unsafe paths and loaded records.
Specify first-start empty state, missing/corrupt existing state and unsupported
platform errors without silently discarding prior publication. Test real temporary
stores, lock contention, truncation, wrong owner and resource cleanup.

Completed: Added no-follow descriptor-anchored owner-only storage, exclusive writer locking, bounded private record encoding and atomic file/directory synchronization; tested reopen, corruption, symlinks and permissions. Retried coverage after a truncated subprocess profile. Required checks pass. Production line coverage: 6243/6563 = 95.12%

### 16. Crash recovery

Exercise each durable boundary before/after writes, synchronization, replacement
and acknowledgement with fault injection and fresh-process reopen tests where
applicable. Recover acknowledged snapshots as complete Board/definition pairs;
unacknowledged commits may resolve only to permitted old/new complete states.
Test I/O failures, leftover staging records, corruption, competing writers and
shutdown during publication. Explain which guarantees derive from real filesystem
checks versus simulated crashes; do not claim power-loss testing from process exit.

Completed: Exercised every durable write boundary, abandoned staging, separate-process reopen and public-owner unknown-outcome reconciliation; proved no automatic recommit and immutable old pins. Required checks pass. Production line coverage: 6307/6563 = 96.10%

### 17. Remote cache provenance

Accept complete bounded documents only through explicit retrieval ports carrying
trusted authenticated publisher identity, receipt time and connection context.
Distinguish directly authenticated publication from forwarded/unattributed data;
identical bytes or an embedded publisher field cannot authenticate the original.
Key by identity rather than endpoint and choose finite freshness/eviction policy.
Use atomic cache replacement with pinned results; test forged context, endpoint
changes, expiry, capacity exhaustion, failed refresh and absent dependencies.

Completed: Added identity-keyed direct/forwarded receipts, trusted source authority, immutable bytes, finite cache and pin budgets, explicit expiry and no hidden refresh operations. Required checks pass. Production line coverage: 6383/6643 = 96.09%

### 18. Remote definition association

Fetch/cache the requested definition against its selected Board snapshot and
validate exact identity fields and schema structure. A remote publication may
change between requests; return stale/inconsistent/missing rather than combining
incompatible revisions. Keep definition association and unsupported schema status
explicit, with bounded retained entries. Test refresh races, removal, wrong IDs,
fetch failure and delayed responses without silently retrying or accepting work.

Completed: Added complete remote definition association with freshness and revision checks, aggregate limits and immutable replacement receipts; delayed responses cannot mix old offerings with a new publication. Required checks pass. Production line coverage: 6447/6705 = 96.15%

### 19. Selection and roles

Select by explicit caller policy and requested identity/contract, returning an
immutable offering/definition pair with usability reasons. Derive requester and
provider from provided/wanted direction and publisher/peer, preserving the explicit
issuer independently of TCP opener. Require understood and satisfied preconditions;
unknown offerings must not hide known selectable ones. Test both directions,
third-party issuers, reordered arrays, unsupported contracts and conservative
custom compatibility without implied array priority or payment authorization.

Completed: Added authority-checked local and direct-remote selection, exact contract requirements, semantic checks and immutable requester/provider/issuer terms independent of TCP direction. Required checks pass. Production line coverage: 6490/6795 = 95.51%

### 20. Staleness and retained terms

Detect changes/removal against the current snapshot and invalidate future
selection; unchanged contract identity does not authorize accepting a new price.
Expose terms for later Services to copy without retaining mutable Board state.
Test price 7 to 9, issuer/definition changes, removal, reordered arrays and pinned
old terms surviving replacement/restart as owned copied data where persisted.
Use a test-owned invocation consumer to prove Board does not cancel obligations;
mark actual quote acceptance and durable Services context as later integration.

Completed: Added current offering/definition checks before local selection, explicit stale results after repricing or removal and public-contract scenarios proving copied price-7 terms survive price-9 publication and owner drop. Required checks pass. Production line coverage: 6572/6836 = 96.14%

### 21. Local exercise commands

Add bounded offline Board validation and public inspection through explicitly
selected files/stores and local authority. Choose and document command/option
spellings, units, defaults and errors before wiring them; every spelling needs
help <topic>, parser tests and executable scenarios. Validation reports format
validity separately from local publishability and never registers fake services.
Do not add a production publish path that bypasses absent provider/requester
capabilities; exercise publication through the library integration harness until
real composition can supply them. Keep stdout intentional and failures on stderr.

Completed: Added bounded offline board-validate and protected board-show commands with explicit publisher checks, format-versus-publishability output, discoverable help for every option and executable malformed-input tests. Required checks pass. Production line coverage: 6654/6923 = 96.11%

### 22. Isolation, concurrency and logging

Test cross-owner references, revoked grants, unsupported contract versions,
cancelled/deadline-expired operations, reader/publisher/cache races and bounded
retained snapshots. Capture TRACE on success/failure and ensure no raw document,
description, suggestion, secrets or unbounded peer diagnostics leak into logs.
Prove untrusted hints cause no network, installation, signing, execution or
spending effects. Confirm reader/cache failures do not corrupt local publication.

Completed: Added barrier-driven publication/read races, revoked/cancelled operation checks, expired issuer denial and TRACE payload exclusion. Fixed single-revision admission to release unpinned current state and rechecked capabilities under writer serialization. Required checks pass. Production line coverage: 6684/6946 = 96.23%

### 23. Acceptance evidence

Run every applicable BD-01–BD-10 row and AT-C01–AT-C04 through public direct-call
ports with literal expected values, real Identity parsing and real persistence
where claimed. Identify each injected dependency and unsupported deployment.
BD-09's real quote/payment acceptance remains deferred to Services/Peer Protocol;
record Board-level evidence separately rather than marking that integration passed.
Resolve implementation gaps, link protocol blockers, and verify any upstream fixes
before updating local reports. Do not claim a useful node from valid empty Boards.

Completed: Added explicit remaining BD matrix rows through public contracts, including issuer aliases, every duplicate placement, unknown siblings, definition version mismatch, array reordering and exact decimal/exponent integer arithmetic. Published assertion-level evidence with BD-09 quote integration explicitly deferred. Required checks pass. Production line coverage: 6723/6980 = 96.32%

### 24. Documentation and handoff

Publish module README, public companion contracts, acceptance matrix and runnable
library/CLI examples from the repository root, including build/test commands,
limits, persistence recovery, authority, provenance and capability restrictions.
Update root README, overview/detailed help and roadmap with actual support only.
Run all required checks, API docs, command scenarios and production coverage;
record the measured percentage. Mark Board's implemented contract scope complete
only with passing evidence and explicit deferred integration rows, then hand off
to Ledger without expanding or implementing that next plan in this task.

Completed: Completed implemented Provider/Cache contracts, independent bounded Services schema formats, exact known-contract validation, API examples and current guides. Audited effect-boundary checks, snapshot reclamation and private record overflow. All 26 help spellings and API docs pass; BD-09 quote/payment integration and namespace grammar remain explicitly tracked for later modules/upstream clarification. Required checks pass. Production line coverage: 6752/7017 = 96.22%
