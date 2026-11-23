# Static Pages and Hosted Publications plan

Status: in progress (steps 1-11 complete).

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
- [ ] **12.** Add the Services provider adapter and requester client with durable replay.
- [ ] **13.** Add Static Pages canonical paths and media types.
- [ ] **14.** Add the page manifest codec and hosted page assembly on the reader side.
- [ ] **15.** Add latest-version selection and equivocation detection across hosts.
- [ ] **16.** Add the publisher-served `cr2se.page` provider with its offering and get operation.
- [ ] **17.** Bind both services to the Peer Protocol endpoint and the node composition.
- [ ] **18.** Add the publisher publish and reader fetch commands with help topics.
- [ ] **19.** Exercise a phone-style publisher with two hosts and one reader over authenticated connections.
- [ ] **20.** Record the acceptance matrix HP-01..15 and PG-01..12, run all checks and update the roadmap.

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

### 12. Services adapter and client

Provider and requester traits for Services, operation keys, replay cache and
crash recovery like Messaging steps 30 to 33. Tests: HP-15 definitions and
independence of replicas.

### 13. Paths and media types

Static Pages sections 3 and 4: canonical paths and media type validation.
Tests: PG-03, PG-04.

### 14. Manifest and page assembly

Canonical manifest bytes, root `/` text/html, per-resource checks and the
incomplete-page rule. Tests: HP-08 and its rejection list.

### 15. Latest version and equivocation

Reader-side selection of the highest verified sequence across hosts,
equivocation report, best-known wording. Tests: HP-10.

### 16. Publisher-served page

`cr2se.page` version 1 get operation, fixed price, immutable snapshot, local
check. Tests: PG-01 to PG-12 for the supported subset.

### 17. Peer Protocol and node binding

Register both services with the node, publish the offerings after recovery, and
bind requester intents. Tests: authenticated in-process node pair.

### 18. Commands and help

`talksphere publish ...` and `talksphere page ...` commands with detailed help
topics, README examples and CLI tests (exit status 2 for invalid arguments).

### 19. Exercise

An example and `docs/pages/EXERCISE.md`: a publisher that only dials out stores
a signed page at two hosts, one host stops, a reader still fetches and verifies
it; a restricted page is readable only by its audience. As with Messaging, nodes
may share one OS process; say so in the example source.

### 20. Acceptance and wrap-up

Matrix HP-01..15 and PG-01..12 with evidence, fmt/clippy/test/build/help,
production coverage above 80% per `docs/coverage.md`, README and roadmap
updates, and a handoff note for milestone 16.
