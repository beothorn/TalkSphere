# Static Pages and Hosted Publications plan

Status: in progress (steps 1-23 complete).

This is roadmap milestone 15 and planning folder 15. It implements the CR2SE
[Hosted Publications](https://github.com/beothorn/CR2SE/blob/53f12451859e0ad207ade9b8a23be20ceb404495/modules/hosted-publications/README.md)
proposal ([beothorn/CR2SE#14](https://github.com/beothorn/CR2SE/issues/14), upstream acceptance pending) for
**pages**, and the existing publisher-served
[Static Pages](https://github.com/beothorn/CR2SE/blob/53f12451859e0ad207ade9b8a23be20ceb404495/modules/pages/README.md)
(`cr2se.page` version 1). Hosted feeds reuse the publication engine built here
and belong to milestone 16.

Baseline: CR2SE `53f1245` (Hosted Publications specification, vectors and
acceptance cases HP-01 to HP-15) over `73edf2d` for the existing modules.
Hosted Publications is not accepted upstream yet; if upstream changes the
specification, recheck every step against the new revision before relying on it.

## Layout decision

- `src/modules/hosted_publications/` owns the service `cr2se.hosted-publication`
  version 1: publication signing, envelopes, terms and pricing, placements,
  provider operations and the reader checks. Pages and feeds both use it.
- `src/modules/pages/` owns canonical paths, media types, page manifests,
  hosted page assembly on the reader side and the publisher-served `cr2se.page`
  service.
- Hosted Publications reuses Messaging's rules by reference and its pricing
  helpers through explicit contracts; it never reads Messaging's private state.

## Steps

- [x] **1.** Record the Hosted Publications and Static Pages requirements, owners and gaps in a reconciliation file.
- [x] **2.** Add the hosted publication module scaffold, identifiers and typed errors.
- [x] **3.** Implement canonical publication metadata and the publication signing bytes with the V1 to V3 vectors.
- [x] **4.** Implement publication verification against the authenticated publisher.
- [x] **5.** Implement the restricted envelope layout, encryption and opening with the V3 vector.
- [x] **6.** Add the Board `cr2se.hosted-publication` offering terms and exact pricing.
- [x] **7.** Add the durable placement record and store with its state machine.
- [x] **8.** Implement the store operation with sequence and duplicate rules.
- [x] **9.** Implement list with visibility, ordering and cursors.
- [x] **10.** Implement get with bounds and indistinguishable denials.
- [x] **11.** Implement renew, remove with release credits, and the byte check.
- [x] **12.** Add the Services codec, the pinned service definition and its catalogue.
- [x] **13.** Add the participant journal and recovery of prepared effects.
- [x] **14.** Add the Services provider adapter with staged results and finalization.
- [x] **15.** Add the requester client with exact pricing, result validation and check rules.
- [x] **16.** Recover across Services, Ledger and hosted files after crashes at every boundary.
- [x] **17.** Add Static Pages canonical paths and media types.
- [x] **18.** Add the page manifest codec and hosted page assembly on the reader side.
- [x] **19.** Add latest-version selection and equivocation detection across hosts.
- [x] **20.** Add the publisher-served `cr2se.page` provider with its offering and get operation.
- [x] **21.** Bind both services to the Peer Protocol endpoint with requester intents and real-endpoint tests.
- [x] **22.** Add the pages example command and its help topics.
- [x] **23.** Exercise a phone-style publisher with two hosts and one reader over authenticated connections.
- [ ] **24.** Record the acceptance matrix HP-01..15 and PG-01..12, run all checks and update the roadmap.

## Details

### 1. Reconciliation

Write `src/modules/hosted_publications/REQUIREMENTS.md` mapping each HP and PG
case to an owner module and step, listing gaps (for example the Board needs no
change, the Services catalogue needs the new definition). Done when every case
has an owner and step. Test: none beyond a link check.

Completed: `REQUIREMENTS.md` maps HP-01..15 and PG-01..12; HP-09 waits for milestone 16.

### 2. Scaffold, identifiers and errors

Create the module, `mod.md`, README, typed `Error` (Invalid, Denied, Conflict,
Limit, Unavailable, Storage), ID newtypes with exact 32-byte checks, and a
`Clock`/`Entropy` port. Unit tests cover length checks and error classes.

Completed: `ids.rs` reuses Messaging's placement, removal and signature types; `error.rs` maps to Services and Messaging.

### 3. Signing bytes

`Publication` metadata and `signing_bytes`, big-endian integers, exact
constants, audience order check. Tests reproduce V1, V2 and V3 signing bytes and
SHA-256 and the Ed25519 signatures byte for byte, and mutate every field.

Completed: `metadata.rs`, `signing.rs`; vectors are copied to `fixtures/vectors.md` and reproduced byte for byte.

### 4. Verification

`verify(publication, expected_publisher)`: derive the identity ID from the
public key, compare, validate kind, sequence rule (resource sequence zero),
visibility and audience, size and hash, then the signature. Tests cover each
rejection and the bounds; one `Invalid` error names no field so callers cannot probe which rule rejected a value.

Completed: `verify.rs` with `Bounds`.

### 5. Envelope

Layout of section 5: header 61 bytes, entries 104 bytes, ciphertext at least 16.
Use Cryptography X25519, HKDF-SHA-256 and XChaCha20-Poly1305. Tests reproduce
every V3 byte, open with the reader key, and reject each HP-07 failure.

Completed: `envelope.rs` reproduces the 295-byte V3 envelope; Messaging's `PersistentAgreement` lets a protected Identity open it.

### 6. Terms and pricing

Offering `hosted-publication-standard` parsed and validated; limits ordered;
`storePrice`, `getPrice`, `renewPrice` with exact rational ceiling arithmetic.
Tests use the HP pricing (minimum 1, factors 1/2) and overflow edges. The pinned service definition and its Services catalogue are part of step 12.

Completed: `terms.rs` and `fixtures/offering.json`; the finite profile caps content at 16384 bytes (see REQUIREMENTS.md).

### 7. Placement store

Durable placement record (pending, removed, expired), protected file storage
following Messaging's pattern, bounded tables, duplicate `(publisherId,
publicationId)` guard. Tests cover restart and bounds.

Completed: `placement.rs`, `store.rs` (duplicate and same-sequence rules enforced atomically at insert) and `lifecycle.rs` with thread races; Messaging's owner-only file helpers are shared.

### 8. Store operation

Validate, price, check sequence conflicts, persist before success. Tests: HP-02,
HP-03 including no charge on every rejection.

Completed: `operation.rs` and `provider.rs` (`assess_store` then `commit_store`, idempotent for crash repair).

### 9. List

Visibility filter, `(sequence, publicationId)` descending order, exclusive
cursor, `more` flag, indistinguishable shape for hidden items. Tests: HP-04, HP-05 (the equal-sequence pair of HP-05 cannot be stored; REQUIREMENTS gap 3).

Completed: `Provider::list` with `ListRequest`, `ListEntry` and `ListOutput`.

### 10. Get

`maximumBytes` bound, readable lookup, exact stored bytes, price. Tests: HP-06
and the unknown/expired/removed/unauthorized indistinguishability.

Completed: `Provider::get` with `GetRequest` and `GetOutput`.

### 11. Renew, remove, check

Reuse Messaging's rules by contract: guarded renewal with total ceiling, removal
quote with release credits and idempotent replay through the Ledger
`issue_release`, one-byte check with local verdict. Tests: HP-11 to HP-14.

Completed: `provider/publisher.rs` with renew, remove and check; the requester-side verdict of the check arrives with the client in step 12.

### 12. Codec, definition and catalogue

Typed requests and responses over the Services value model (exact field sets per operation, unknown fields rejected, forbidden fields of another
operation rejected, the `store` body and `content` as bytes), the pinned service definition from the proposal with its Services catalogue like
Messaging's `standard.rs`, and the partial-cursor and kind/limit input rules. Tests: HP-15 definition fields, field-set rejections, round trips.

Completed: `codec.rs`, `standard.rs` and `fixtures/definition.json` (extracted from the proposal's service-definition section).

### 13. Participant and recovery

Effects (`Query`, `Store`, `Renew`, `Remove`) and outcomes with strict durable encoding, a bounded journal of prepared, committed and aborted
operations, the participant's prepare, commit, abort and status with per-placement locks and idempotent apply, and recovery that defers to
Services. Tests: the Messaging participant scenarios (locks, token replay, crash between apply and journal, corrupt journal refused).

Completed: `effect.rs`, `journal.rs`, `participant.rs` (locks on placement, publication and sequence slot) and `recovery.rs`.

### 14. Services provider adapter

`ServiceProvider` mapping stage, prepare, finalize and commit onto the engine; stores finalized with the coordinator's decision time; read-only
operations keep their request in the journal; cached retry only for removals. Tests: staged results after restart, finalization, evidence.

Completed: `service/{mod,provider,project,outputs}.rs`; header-only store assessment, `Provider::assess_header` and `readable_content_size`. The end-to-end coordinator rig arrives with the client (step 15).

### 15. Requester client

`Client` with exact pricing from the offering (store by size and retention, get by the returned size bounds, renew by the known size, removal by
the quoted release credits), result validation (the publication belongs to its request, expiry arithmetic) and the check rules with a private
expectation. Tests: HP-06, HP-11 to HP-13 pricing and verdicts, hostile results.

Completed: `service/client.rs`, `check.rs` and a full lifecycle through the real Services coordinator, Board and Ledger with exact charges (store 17, list 1, get 2, renew 8, remove 2 with 12 credits released).

### 16. Crash recovery

Resume unfinished operations from the coordinator's record across Services, Ledger and hosted placements, including release credits issued once.
Tests: a crash injected at every boundary of store, renew and remove, with exactly-once effects (HP-12).

Completed: `service/recovery.rs` and `service/crash_tests.rs` (store, renew, remove and get at ten and eight coordination boundaries over three file directories).

### 17. Paths and media types

Static Pages sections 3 and 4: canonical paths and media type validation.
Tests: PG-03, PG-04.

Completed: `pages/{error,path,media}.rs`.

### 18. Manifest and page assembly

Canonical manifest bytes, root `/` text/html, per-resource checks and the
incomplete-page rule. Tests: HP-08 and its rejection list.

Completed: `pages/manifest.rs` (V2 vector reproduced) and `pages/hosted.rs` (build and assemble, public and restricted).

### 19. Latest version and equivocation

Reader-side selection of the highest verified sequence across hosts,
equivocation report, best-known wording. Tests: HP-10.

Completed: `hosted_publications/latest.rs` (shared by pages and feeds).

### 20. Publisher-served page

`cr2se.page` version 1 get operation, fixed price, immutable snapshot, local
check. Tests: PG-01 to PG-12 for the supported subset.

Completed: `pages/service/` (terms, codec, check, store with a confined directory source, definition catalogue, provider adapter and client) with a coordinator test of exact charging.

### 21. Peer Protocol binding

Requester intents (`Remote`) with exact spending ceilings for both services, the Peer Protocol quote bindings the hosted service needs (`contentSize` pricing input for get and renew, release credits for remove, replay of removals,
the check evidence shape) and tests over real endpoints with three independently owned stacks. Installing the families in the node composition belongs to the
[client journey](../14_Client_journey/PLAN.md), like Messaging's.

Completed: `hosted_publications/wire.rs`, `pages/service/wire.rs`, the Peer Protocol and Services replay hooks and `engine/endpoint/tests/{hosted_publications,pages}.rs`.

### 22. Command and help

The `examples/pages` command (`run`, `help run`) with detailed `talksphere help pages` (also `hosted-publications`) help, README examples and CLI tests (exit status 2 for invalid arguments). A real
publisher or reader command over a node needs the node composition, which belongs to the [client journey](../14_Client_journey/PLAN.md), as for Messaging.

Completed: `examples/pages.rs`, `src/cli/help.rs` topic and `tests/cli.rs`.

### 23. Exercise

An example and `docs/pages/EXERCISE.md`: a publisher that only dials out stores
a signed page at two hosts, one host stops, a reader still fetches and verifies
it; a restricted page is readable only by its audience. As with Messaging, nodes
may share one OS process; say so in the example source.

Completed: `examples/pages/{owners,exercise}.rs` and `docs/pages/EXERCISE.md`; the run passes over real authenticated loopback TCP.

### 24. Acceptance and wrap-up

Matrix HP-01..15 and PG-01..12 with evidence, fmt/clippy/test/build/help,
production coverage above 80% per `docs/coverage.md`, README and roadmap
updates, and a handoff note for milestone 16.
