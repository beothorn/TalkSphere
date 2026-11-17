# Storage module plan

Status: implementation authorized; steps 1–16 and 18–22 are complete; step 17 awaits upstream acceptance-time clarification in issue #11.

This is roadmap milestone 10 and planning folder 9, following Peer Protocol.

## Steps

- [x] **1.** Reconcile Storage requirements with the existing module contracts.
- [x] **2.** Define Storage types and dependency ports.
- [x] **3.** Add deterministic Storage test helpers.
- [x] **4.** Import pinned Storage definitions and independent fixtures.
- [x] **5.** Implement exact Storage price calculations.
- [x] **6.** Validate Storage offering terms and advertised maxima.
- [x] **7.** Validate operation-specific inputs and outputs.
- [x] **8.** Implement owner-authorized lease lookup.
- [x] **9.** Implement retention clocks and terminal lease states.
- [x] **10.** Reserve bounded capacity for pending Storage work.
- [x] **11.** Persist bounded upload staging and immutable byte handles.
- [x] **12.** Persist lease metadata and retained operation results.
- [x] **13.** Implement durable participant preparation and status queries.
- [x] **14.** Implement idempotent participant commit and abort.
- [x] **15.** Recover Storage participants before admitting new work.
- [x] **16.** Assess store metadata before receiving upload bodies.
- [ ] **17.** Stage validated uploads as durable lease proposals.
- [x] **18.** Validate store results against requester expectations.
- [x] **19.** Assess retrieval ranges and pin accepted bytes.
- [x] **20.** Return exact ranges through retained result handles.
- [x] **21.** Validate retrieved bytes and whole-object hash metadata.
- [x] **22.** Assess renewal guards and exact pricing factors.
- [ ] **23.** Commit serialized lease extensions.
- [ ] **24.** Implement charged logical lease removal.
- [ ] **25.** Reclaim expired and removed lease resources safely.
- [ ] **26.** Implement authorized paid byte evidence.
- [ ] **27.** Verify byte challenges using private expectations and trusted timing.
- [ ] **28.** Register Storage handlers and publish usable offerings.
- [ ] **29.** Bind Storage requester operations to Peer Protocol.
- [ ] **30.** Support wanted Storage with requester-owned consent.
- [ ] **31.** Verify Storage accounting across supported payment modes.
- [ ] **32.** Exercise crash recovery at every coordination boundary.
- [ ] **33.** Verify concurrent operations and resource isolation.
- [ ] **34.** Exercise persistent Storage between authenticated node processes.
- [ ] **35.** Exercise encrypted recovery from independent providers.
- [ ] **36.** Add a discoverable Storage exercise and detailed help.
- [ ] **37.** Complete the Storage acceptance matrix.
- [ ] **38.** Publish the Storage completion and minimum-node handoff.

## Baseline, scope and review

Reviewed on 2026-11-15 against
[CR2SE d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
Upstream `main` is six commits ahead, with no changes to the reviewed protocol
files. Recheck the specification and issue status before implementation.

- [Storage specification, including the complete definition and pricing examples](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md).
- [Input contract](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/inputs/README.md).
- [Output contract](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/outputs/README.md).
- [Pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/pseudocode/README.md).
- [Storage acceptance cases ST-01–ST-13](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/acceptanceTests/README.md).
- [Shared ownership and durable coordination contracts](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md).
- [Shared acceptance and integration cases](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/AcceptanceTests.md).

Use the [Peer Protocol handoff](../../src/modules/peer_protocol/HANDOFF.md),
[Services handler contract](../../src/modules/services/handler.md),
[registry contract](../../src/modules/services/registry.md),
[body ownership contract](../../src/modules/services/body.md), and
[network runtime guide](../../docs/peer-protocol/RUNTIME.md).
Storage owns paid immutable leases, bytes, operation effects and retained results;
Services coordinates decisions, Ledger owns accounting, Board owns publication,
and Peer Protocol owns authenticated exchanges. Storage is not their persistence
backend. Keep implementation under `src/modules/storage/`, split by useful
responsibilities such as contracts, pricing, leases, persistence, provider and
requester; put application wiring under composition. Create directories only
when they contain useful work.

Implement requester and provider behavior for `cr2se.storage` version 1 and
`cr2se.storage.v1`: store, retrieve, renew, remove and separately paid byte checks.
Support current provided offerings for follow-up operations and provided/wanted
initial stores. One lease is one copy at one provider. Requester encryption,
keys and recovery catalogs belong to the application; use the existing
Cryptography and Identity capabilities in recovery tests without creating a new
backup service or optional module.

This plan includes real durable Linux adapters and an explicit runnable Storage
exercise. The subsequent minimum useful node milestone owns complete operator
configuration, authorized local control, contact management and default server
composition. Do not advertise Storage until its complete handler and recovery
path are usable, and do not present the existing Transport-only `serve` command
as a Storage server. Existing bounded in-memory body contracts may support a
finite advertised Storage profile; do not promise arbitrary-size streaming or
expand advertised limits beyond the composition's actual budgets.

[Upstream issue #10](https://github.com/beothorn/CR2SE/issues/10) remains open:
requester check timeout does not establish rollback of a provider's committed
charge. Record each side's observed result and accounting separately; do not
invent an acknowledgement, refund or globally uncharged-timeout guarantee.
The implementation review confirmed [issue #11](https://github.com/beothorn/CR2SE/issues/11): store pseudocode samples acceptance time before durable commit, contrary to the normative acceptance boundary. See the [local evidence report](../../CR2SE%20bugs/0008-storage-acceptance-time-before-durability.md); step 17 remains pending clarification. The original planning review identified no additional defect.
Investigate contradictions before choosing interoperable behavior, search open
and closed upstream issues, and retain confirmed reports and issue URLs under
[CR2SE bugs](../../CR2SE%20bugs/README.md). A local adapter limitation is not a
protocol defect.

Each implementation step includes unit tests for success, invalid boundaries and
dependency failures, current public Markdown companions, function explanations,
safe structured tracing and applicable integration tests. Run the required
[contributor checks](../../AGENTS.md), update affected documentation/help, and
measure production coverage using the [coverage procedure](../../docs/coverage.md)
to keep it above 80% before marking implementation complete. Commit focused,
verified changes with descriptive titles and bodies under the established
workflow. The final acceptance steps consolidate evidence; they do not postpone
testing. The subsequent user request authorizes all 38 steps. Checked entries require
completed evidence; the requirements audit is linked below.

## Details

### 1. Reconcile Storage requirements with the existing module contracts

Completed: [requirements and public-port audit](../../src/modules/storage/REQUIREMENTS.md).

Create a requirements matrix for ST-01–ST-13, AT-C01–C04 and applicable SV-09
coordination cases. Trace each rule to its source, owner and planned tests.
Review exact acceptance timing, clock correction across restart, charge versus
remote delivery, and the existing issue #10 before coding dependent behavior.
Audit public Services assessment/result/participant ports and Peer Protocol
budgets; record necessary narrow prerequisite changes with regression tests.
Do not read another owner's private journal or silently resolve normative gaps.

### 2. Define Storage types and dependency ports

Define exact IDs, hashes, terms, operation inputs/results, lease snapshots,
authorized contexts, typed failures and finite configuration. Inject clock,
randomness, Cryptography hashing and durable byte/metadata ports. Separate
requester verification from provider effects and platform persistence. Add a
scaffold README and contract companions with unsupported behavior explicit;
test widths, authority boundaries and dependency failures.

### 3. Add deterministic Storage test helpers

Provide controlled wall/monotonic clocks, random collisions/failures, bounded
byte sources, crashable durable stores, lost acknowledgements and barriers.
Count executed operations, durable writes and correlated financial effects.
Reconstruct from acknowledged durable state rather than reusing live objects;
exercise script exhaustion and avoid sleeps or oversized allocations.

### 4. Import pinned Storage definitions and independent fixtures

Retain the complete section 13 definition and representative provided/wanted
terms with source revision and provenance. Import ST-01 literal prices, the
ASCII `abc` SHA-256 vector and exact input/output examples; independently derive
additional boundary arithmetic expectations. The specification and acceptance
suite provide these fixtures rather than a separate assumed Storage vector file.
Keep standard-definition validation independent of the implementation's encoder.

### 5. Implement exact Storage price calculations

Calculate store with one ceiling after adding both rational terms, retrieve and
renew with their specified factors, and remove/check with exact fixed prices.
Use arbitrary precision or a proven equivalent; uint128 alone does not cover
all possible intermediates. Test zero numerators, positive minima, unequal
denominators, prices above 2^53, uint64 boundaries and final overflow against
independent expectations, including ST-01 prices 1, 17, 2, 6, 2 and 3.

### 6. Validate Storage offering terms and advertised maxima

Require the exact service/version/model, complete pricing/info fields, positive
limits/checkPrice/removePrice/minimumPrice and identity precondition; reject
fixed `price`, zero denominators and inconsistent retention bounds. Validate
store/retrieve/renew prices at advertised maxima before publication or selection.
Preserve common compatible-extension behavior and immutable accepted terms.
Test both provided and wanted shapes and current follow-up offering replacement.

### 7. Validate operation-specific inputs and outputs

Implement the four invocation discriminants and separate remote check schema.
Require the correct fields and fixed byte lengths; reject known fields from
other operations while retaining permitted unknown extensions. Preserve exact
uint64 values and distinguish unresolved body metadata from complete content.
Validate required successful output fields despite optional fields in the common
schema; test malformed, omitted, forbidden and conflicting values per operation.

### 8. Implement owner-authorized lease lookup

Bind access to authenticated requester and local provider identities, never a
lease ID or hash alone. Return externally indistinguishable missing/foreign
lease failures without protected metadata. Preserve access for the same
coordinated identity on a new connection and reject forged local authority.
Cover every operation under ST-04, including bounded diagnostic/probing behavior.

### 9. Implement retention clocks and terminal lease states

Model receiving, active, expired and removed with checked Unix-second arithmetic.
Start retention at durable acceptance, not quote acceptance or upload start.
Specify wall/monotonic clock and restart behavior that preserves promised
retention through corrections; do not let a later clock rollback reactivate a
terminal lease. Test exact expiry, forward/backward changes, restart, overflow
and requester clock-skew policy; resolve any unsatisfied protocol assumptions
through step 1's issue process rather than silently weakening them.

### 10. Reserve bounded capacity for pending Storage work

Atomically reserve configured bytes, lease slots, pending operations, retained
results and per-owner concurrency before proportional work. Count staged and
pinned data as well as active leases; reject excess declarations without large
allocation. Release provisional capacity on confirmed abort, retain prepared
holds through uncertainty, and distinguish new-work refusal from existing lease
obligations. Test exhaustion, competing reservations and cleanup failures.

### 11. Persist bounded upload staging and immutable byte handles

Implement protected local staging, exact bounded writes, flush/durability barriers
and owner-issued immutable range handles. Avoid peer-controlled paths, symlink
traversal, leaked handles and process-pointer references. Test short writes,
partial reads, disk/flush failure, interruption and reopening after restart.
Reuse bounded existing body contracts where sufficient and document their memory
ceiling; durable files alone do not make the network input path streaming.

### 12. Persist lease metadata and retained operation results

Persist identities, accepted terms, size/hash, times, original price, terminal
state and byte associations alongside operation/effect/result records. Use a
versioned bounded representation with atomic replacement or a recoverable journal,
protected ownership and explicit corruption/version failures. Keep enough ID
history to prevent reuse within a provider identity. Test interrupted persistence,
missing bytes, corrupt metadata and exact uint64 round trips across restart.

### 13. Implement durable participant preparation and status queries

Stage effects and results under the original Services operation key and bind
opaque preparation tokens to owner, key and exact effect. Repeated identical
preparation/status must recover the same answer; conflicting reuse fails.
Prepared holds and result references survive process restart without becoming
visible successful leases. Test lost preparation replies, foreign tokens and
unavailable persistence through public ports.

### 14. Implement idempotent participant commit and abort

Apply only the coordinator's immutable decision, at most once, and retain
recoverable final status and output. Abort releases uncommitted resources without
charge; it never reverses committed state or follows merely from timeout.
Gate conflicting access while participant effects remain incomplete. Test repeated
and conflicting decisions, failure between writes and lost commit acknowledgements.

### 15. Recover Storage participants before admitting new work

Reopen leases, staged effects, retained results and capacity accounting, then
resolve pending decisions through the existing Services recovery contract.
Block affected resources until resolution; never rerun execution or infer abort
from an absent reply. Preserve handlers for accepted obligations even after
new publication stops. Test unavailable coordinators, repeated recovery and
restart with receiving, prepared, committed, expired and removed state.

### 16. Assess store metadata before receiving upload bodies

Implement provider assessment and requester metadata projection for positive
size, retention, declared content hash, exact price and accepted spending bound.
Support unresolved body descriptors without placeholder data or early execution.
Tie capacity pins to their accepted context and release rejected proposals.
Test invalid limits, stale publication, price disagreement and refusal before body
execution, while leaving lease creation to durable completion.

### 17. Stage validated uploads as durable lease proposals

Pending: resolve [upstream issue #11](https://github.com/beothorn/CR2SE/issues/11) before choosing actual acceptance timestamps and completing this step.

After accepted terms and reservation, require complete input and exact SHA-256,
length and terminal markers before staging a lease. Generate unpredictable unique
32-byte IDs with bounded collision handling and preserve immutable logical bytes.
Record durable acceptance time and checked expiry consistently with coordination.
Exercise ST-02/03, random/storage failure, truncated/excess upload and hash mismatch;
no active obligation or charge is exposed for a confirmed uncommitted failure.

### 18. Validate store results against requester expectations

Independently verify operation, lease ID, original size/hash, accepted price,
retention equation and acceptable clock difference before requester commitment.
Retain the successful lease summary through the requester application's catalog
contract; do not fabricate a catalog at the provider. Test mismatched output,
late/truncated response and unknown commitment without automatic new stores.

### 19. Assess retrieval ranges and pin accepted bytes

Authorize an active lease and current compatible provided offering, then validate
positive length, current maximum and `length <= size - offset` with `offset < size`.
Pin the exact immutable range at timely operation acceptance, not merely during
an abandoned quote; preserve it across later expiration/removal. Test ST-05/06,
quote delay across expiry, publication withdrawal and rejected-pin cleanup.

### 20. Return exact ranges through retained result handles

Produce exactly the requested bytes, original offset, whole-object hash and
agreed price through bounded retained results. An accepted transfer completes
after expiration or removal without permitting new operations. Detect missing or
short backing data and distinguish uncommitted failure from committed reply loss.
Test byte boundaries, backpressure, handle lifetime, restart and storage failures.

### 21. Validate retrieved bytes and whole-object hash metadata

Check lease, offset, length, accepted price and retained whole-object hash before
accepting the complete result. Rehash a full-object retrieval; a partial range
cannot be authenticated by hashing it against the whole-object hash, so compare
available requester-owned expectations without claiming a nonexistent range proof.
Test wrong metadata/bytes, incomplete output and retained results across caller
loss, keeping provisional bytes out of successful application output.

### 22. Assess renewal guards and exact pricing factors

Authorize an active lease, matching expected expiry and current duration/total
limits using checked additions. Supply retained `sizeBytes` for the quote and
independently compare it with requester-owned lease information. Test missing,
extra, forged and uint64-sized price factors without allocating corresponding
objects; rejected terms leave the original lease untouched.

### 23. Commit serialized lease extensions

Recheck the accepted guard at the durable mutation boundary and atomically retain
previous/new expiry and exact price. Only one renewal with a given expected
expiry succeeds; a lost response does not authorize another extension or replayed
success. Validate requester output independently. Exercise ST-07, persistence
failure, stale retries, expiry while pending and both renew/remove orderings.

### 24. Implement charged logical lease removal

Authorize and stage removal of an active lease at the current removePrice, then
commit terminal state through the participant. Validate `removed:true`, lease ID
and price on the requester. Preserve already accepted read pins; deny later
operations and repeated removal without a second charge. Test ST-08 and failures,
without refunds, claims of physical erasure or remote cached-retry guarantees.

### 25. Reclaim expired and removed lease resources safely

Make logical expiry terminal at the defined boundary and reclaim bytes only when
accepted transfers, prepared effects and retained result obligations allow it.
Bound cleanup batches and preserve replay/status records for the required lifetime.
Test crash during cleanup, failed deletion, restarted accounting and clock rollback;
physical cleanup failure must not reactivate a lease or discard a live pin.

### 26. Implement authorized paid byte evidence

Use a current provided offering and separately accepted positive checkPrice.
Authorize active ownership and offset, return only leaseId/offset/one byte, and
retain accepted bytes through expiry/removal. Apply the specified optional overlap
refusal and one-second minimum interval without hidden stricter frequency limits.
Test ST-10/11, unavailable backing bytes, refusal and exact evidence shape;
Storage checks are never an included store allowance.

### 27. Verify byte challenges using private expectations and trusted timing

Keep expected bytes requester-local, optionally sample offsets with secure
unbiased randomness, and use original Peer Protocol challenge-send/evidence/end
observations rather than delayed worker execution times. Produce pass, fail or
inconclusive for matching, incorrect, missing, late or expiration-ambiguous evidence.
Test exact timeout boundaries and missing expectations. Preserve separate requester
and provider accounting evidence for issue #10 without automatic trust/refund changes.

### 28. Register Storage handlers and publish usable offerings

Install the complete independently recognized standard definition and actual
requester/provider handlers through Services. Compose recovered durable Storage,
Board and Ledger owners through public contracts, publishing only usable limits.
Test current-term changes, withdrawn offerings, drained handlers and obligations
that remain after publication changes; remote descriptions never install code.
Do not enable an incomplete intermediate provider in the normal server profile.

### 29. Bind Storage requester operations to Peer Protocol

Construct locally authorized store/retrieve/renew/remove/check plans with explicit
input and spending consent, authenticated provider selection and bounded body
ownership. Feed complete results into independent Storage verification and durable
requester coordination. Test detached bodies, final markers, stream failure and
changed terms; generic Storage cannot opt into Messaging's cached-retry path.

### 30. Support wanted Storage with requester-owned consent

Allow an initial store through a wanted offering while the publisher retains
input, encryption and spending authority. Preserve requester/provider/issuer roles
when the provider opens the stream; later operations select current provided
terms at the actual lease provider. Test unauthorized invitations, finite
requester-issued credits and the contribution path described in AT-E02.

### 31. Verify Storage accounting across supported payment modes

Exercise provider-issued, requester-issued and explicitly supported third-party
credit policies using real Services/Ledger contracts. Verify one exact local charge
per committed operation, finite reservations and refusal before unauthorized work.
For third-party support, test exact fee/total bounds and independent issuer
confirmation under AT-E06; payment uncertainty never reexecutes or removes a
committed lease. Document and test explicit refusal for unsupported policies.

### 32. Exercise crash recovery at every coordination boundary

Run SV-09 and AT-E04 with real Storage bytes/metadata and independent durable
Services/Ledger owners, crashing before/after preparation, decision, participant
application and acknowledgement. Recover exact effects/results once, including
reads and mutation conflicts. Separately lose remote success and assert unknown
outcomes without a new invocation; identical content in an explicitly new store
creates a separate lease rather than an implicit deduplication key.

### 33. Verify concurrent operations and resource isolation

Use deterministic barriers for same-expiry renewals, renew/remove and read/check
versus removal/expiration. Exercise quota races, slow consumers, multiple owners,
shared endpoints, retained results and shutdown with accepted work. Assert valid
serialized histories, finite accounting and sibling progress, plus safe bounded
logs without keys, raw content or foreign lease metadata.

### 34. Exercise persistent Storage between authenticated node processes

Extend real node/process harnesses with actual Storage, protected Identity,
Cryptography, Board, Services and Ledger adapters. Store nonempty and detached
payloads, restart the provider, retrieve verified bytes, then renew/check/remove
under current terms with optional families absent. Include unauthorized peers,
interrupted uploads and lost output. Record genuine process/disk evidence separately
from injected crash tests and from independent-implementation interoperability.

### 35. Exercise encrypted recovery from independent providers

Cover ST-13 and AT-E03 with requester-side authenticated encryption, separate B/C
leases and independently retained keys/catalog. Discard requester working data,
take B offline, restore the same coordinated requester identity and recover,
hash-check and decrypt from C. Demonstrate that repeated leases at one identity
do not establish independent redundancy and keys never reach providers. Keep
this an application integration fixture, not a new Storage replication service.

### 36. Add a discoverable Storage exercise and detailed help

Provide an explicit runnable persistent Storage example with finite budgets,
trusted contacts and isolated state directories; no-argument invocation shows
help without creating state or opening sockets. Document build/run/test, restart,
all operations, logging, cleanup and the limits of one provider copy. Update root
and module READMEs, overview/detailed CLI discovery and tests for options and exit
statuses together; preserve the separate minimum-node composition milestone.

### 37. Complete the Storage acceptance matrix

Map every ST-01–ST-13 assertion and relevant shared/Services case to executed tests,
fixtures, adapter scope and results. Record the exact issue #10 blocked global
claim rather than counting a requester-only timeout as global conformance. Run
required formatting, Clippy, tests, build, help, example and rustdoc checks; measure
production coverage above 80%. Audit links, public companions, logs and advertised
capabilities, and resolve local regressions before declaring completion.

### 38. Publish the Storage completion and minimum-node handoff

Reconcile README, roadmap and detailed checklist against actual implemented and
verified scope. Publish the supported platform/durability profile, finite limits,
recovery commands, remaining upstream restrictions and integration contracts for
the minimum useful node. Distinguish a complete Storage module from a deployed
minimum node and full-feature PoC; preserve later milestones unchecked and do not
expand or implement their plans as part of this step.
