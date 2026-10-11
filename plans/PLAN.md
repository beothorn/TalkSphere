# Implementation plan

Implement one module at a time against
[CR2SE](https://github.com/beothorn/CR2SE). The initial baseline is revision
[`8e848eb2e70ef7cd4490a753f9e647fdddcd99fd`](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd).
Recheck the upstream module contracts and acceptance tests at each step.

A module is complete only when its contracts and failure behavior are
implemented, unit tests and applicable upstream vectors/acceptance cases pass,
and build/run documentation and discoverable CLI help are current. Document
language-independent protocol defects in [`CR2SE bugs/`](../CR2SE%20bugs/README.md).
Use dependency substitutes to preserve module-by-module development; substitutes
do not demonstrate network authentication or interoperability.

The first module to implement has a
[detailed Transport plan](1_Transport_module/PLAN.md), with single-sentence
checklist steps followed by matching details in the same order. Its folder
number starts at 1 because project setup is already complete; Transport remains
step 2 in this roadmap. No other module has been broken down yet.

Transport substeps 1–2 (public interface, replaceable dependencies and deterministic
test helpers) are complete; the remaining substeps and the overall Transport
milestone are still pending.

## Setup and required node modules

- [x] **1. Project setup.** Cargo application/library, module layout, Rust
  gitignore, contributor instructions, protocol bug directory/template, CLI
  overview and detailed help, unit/integration tests, build/run instructions,
  editor tasks and an explicitly unimplemented Transport scaffold.
- [ ] **2. [Transport module](1_Transport_module/PLAN.md).** Implement bounded frame codecs, fragmented and
  coalesced I/O, handshake and integrity orchestration, bidirectional streams,
  exact stream-ID allocation, half-close, cancellation, ping, GOAWAY, deadlines
  and resource limits. Exercise upstream TR-01 through TR-15 with test-owned
  socket, clock, Identity and Cryptography adapters. Keep authentication
  substitutes test-only; real-crypto vector checks remain gated on steps 3–5.
- [ ] **3. Cryptography module.** Implement specified primitives and canonical
  constructions, fixed vectors, key lifecycle, content encryption and tamper
  rejection. Use established cryptographic libraries and bounded inputs.
- [ ] **4. Identity module.** Implement identity creation, protected credentials,
  binary/text formats, contact representation and signing capabilities. Verify
  round trips, normalization, invalid input and expected-identity checks.
- [ ] **5. Transport integration milestone.** Replace test substitutes with
  Identity/Cryptography adapters. Match upstream handshake/frame vectors with
  real cryptography and connect two local nodes in both directions. Reject
  tampering and replay before dispatch; complete TR-01 through TR-15.
- [ ] **6. Board module.** Publish atomic provided/wanted offerings and definition
  snapshots, using Services schema formats without requiring its runtime.
  Validate consistency, unknown metadata and unsupported required semantics.
- [ ] **7. Ledger module.** Implement exact issuer-specific balances, trust,
  finite grants, bounded reservations, settlement, transfer/claim controls,
  durable state and replay protection. Test concurrent spending and restart.
- [ ] **8. Services module.** Implement typed schemas, handler registration,
  authority, quotes/pricing, provided/wanted roles and durable coordination
  with Board/Ledger through contracts. Test failures and unknown outcomes.
- [ ] **9. Peer Control / Peer Protocol module.** Bind metadata, quotes, service
  invocation and ledger control to Transport streams. Implement canonical
  records, detached bodies and exact value codecs; match encoding vectors and
  reject malformed data without silently retrying accepted operations.
- [ ] **10. Storage module.** Implement requester/provider store, ranged
  retrieval, renewal guards, removal, expiration and paid byte checks, with
  authorization, durable leases and hash verification. Keep local persistence
  independent of the paid Storage service.
- [ ] **11. Minimum useful node milestone.** Compose the eight required modules
  with authorized local control and configured contacts. Demonstrate a paid
  nonempty store, restart and verified paid retrieval between two identities,
  then renewal/check/removal, finite credits, failures and concurrent spends.
  Run with every optional module absent; advertise only working capabilities.
- [ ] **12. Full client journey.** Create a full client journey where two clients
  shares storages, get credits and are able to send messages. The server is able to
  save and retrieve a file.

## Optional CR2SE modules and full-feature PoC

These modules are planned for TalkSphere but are optional in CR2SE's minimum
profile. Implement each independently and register only complete capabilities.

- [ ] **12. Node API module.** Add authenticated local client access, versioned
  JSON-line messages, bounded blobs, session-scoped handles, cancellation and
  explicit spending/issuer authority. Test isolation and out-of-order replies.
- [ ] **13. Messaging module.** Add signed/encrypted placement, recipient
  discovery/recovery, receipts, renewal, release/removal, status, checks and
  retries with durable replay guards.
- [ ] **14. Discovery module.** Add signed records, paid atomic indexing, bounded
  iterative lookup, cursors, retention checks and equivocation handling.
- [ ] **15. Public File Sharing module.** Add canonical manifests, signed
  catalogs, paid ranges, resumable multi-host retrieval, safe paths and
  piece/file verification; reuse Discovery record formats through contracts.
- [ ] **16. Public Feeds module.** Add signed publication, attachments, bounded
  paid reads, exclusive cursors and local feed merging.
- [ ] **17. Static Pages module.** Add authorized local publication, path/media
  validation, fixed-price bounded retrieval and immutable response snapshots.
- [ ] **18. Computation module.** Add an explicitly authorized, bounded runtime,
  exact metered pricing/reservations, timeout/error results and local checking.
- [ ] **19. Full-feature PoC and portability.** Complete the upstream feature
  matrix with three identities, independent encrypted replicas, retained keys
  and recovery catalog, recovery on replacement hardware with a provider
  offline, crash injection and uncertain-delivery tests. Cross-check canonical
  bytes with an independent codec/implementation. Publish reproducible build,
  run and restore instructions, version coverage, results and known limits.

Production readiness requires separate evaluation; completing a scaffold or a
minimum-profile demonstration does not complete the full-feature PoC.
