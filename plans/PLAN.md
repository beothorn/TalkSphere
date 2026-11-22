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
step 2 in this roadmap. The next module has a
[Cryptography plan](2_Cryptography_module/PLAN.md), in folder 2 and roadmap step 3;
all 22 steps are complete, including real primitives, bounded recipient envelopes,
secret lifecycle checks and the Transport adapter; see its [acceptance evidence](../src/modules/cryptography/ACCEPTANCE.md).

Transport substeps 1–24 are complete with injected Identity/Cryptography and
real TCP adapter tests. The [acceptance matrix](../src/modules/transport/ACCEPTANCE.md)
records local evidence and later native supplements. The completed
[server integration evidence](../docs/transport-integration/ACCEPTANCE.md) covers
Linux runtime and real peers; later modules remain required for a minimum useful node.

Identity now has a [detailed plan](3_Identity_module/PLAN.md) in module folder 3,
using upstream revision `d40b6278f24430e814d7fa82094f57261bfcac9f`. The detailed
checklist records all 30 completed steps and verified evidence; see its acceptance matrix
for direct binding, protected Linux storage and recovery scope.
Confirmed new protocol defects are also filed upstream under the contributor instructions.

## Setup and required node modules

- [x] **1. Project setup.** Cargo application/library, module layout, Rust
  gitignore, contributor instructions, protocol bug directory/template, CLI
  overview and detailed help, unit/integration tests, build/run instructions,
  editor tasks and an explicitly unimplemented Transport scaffold.
- [x] **2. [Transport module](1_Transport_module/PLAN.md).** Implement bounded frame codecs, fragmented and
  coalesced I/O, handshake and integrity orchestration, bidirectional streams,
  exact stream-ID allocation, half-close, cancellation, ping, GOAWAY, deadlines
  and resource limits. Exercise upstream TR-01 through TR-15 with test-owned
  socket, clock, Identity and Cryptography adapters. Keep authentication
  substitutes test-only; real-crypto vector checks now pass through the step 3 adapter.
- [x] **3. [Cryptography module](2_Cryptography_module/PLAN.md).** Implement specified primitives and canonical
  constructions, fixed vectors, key lifecycle, content encryption and tamper
  rejection. Use established cryptographic libraries and bounded inputs.
- [x] **4. [Identity module](3_Identity_module/PLAN.md).** Implement identity creation, protected credentials,
  binary/text formats, contact representation and signing capabilities. Verify
  round trips, normalization, invalid input and expected-identity checks.
- [x] **5. [Transport integration milestone](4_Transport_integration/PLAN.md).** Compose real Identity with
  the implemented Cryptography adapter and connect two local nodes in both directions.
  Extend the passing cryptographic vectors to full server integration. Reject
  tampering and replay before dispatch; complete TR-01 through TR-15.
- [x] **6. [Board module](5_Board_module/PLAN.md).** Publish atomic provided/wanted offerings and definition
  snapshots, using Services schema formats without requiring its runtime.
  Direct-call implementation and Linux persistence are complete; validate consistency,
  unknown metadata and unsupported required semantics with the
  [Board evidence matrix](../src/modules/board/ACCEPTANCE.md). BD-09 local quote/payment
  integration is supplemented by Services; network exchange remains Peer Protocol work.
- [x] **7. [Ledger module](6_Ledger_module/PLAN.md).** Implement exact issuer-specific balances, trust,
  finite grants, bounded reservations, settlement, transfer/claim controls,
  durable state and replay protection. Direct-call implementation, protected Linux
  persistence, inspection and [Ledger evidence](../src/modules/ledger/ACCEPTANCE.md)
  cover concurrent spending and restart; Services verifies local coordination and
  Peer Protocol now exercises independent authenticated network controls/settlement.
- [x] **8. [Services module](7_Services_module/PLAN.md).** Implement typed schemas, handler registration,
  authority, quotes/pricing, provided/wanted roles and durable coordination
  with Board/Ledger through contracts. Local direct/serialized coordination, protected
  recovery and [acceptance evidence](../src/modules/services/ACCEPTANCE.md) are complete;
  the paid-check timeout ambiguity in upstream issue #10 remains explicit. Network
  application binding is now verified by Peer Protocol; standard-family providers
  remain later milestones.
- [x] **9. [Peer Control / Peer Protocol module](8_Peer_Protocol_module/PLAN.md).** Bind metadata, quotes, service
  invocation and ledger control to Transport streams. Implement canonical
  records, detached bodies and exact value codecs; match encoding vectors and
  reject malformed data without silently retrying accepted operations. All38 detailed
  steps are complete; the [acceptance matrix](../src/modules/peer_protocol/ACCEPTANCE.md)
  covers real custom-node/process work, checks, wanted roles, independent issuer
  settlement and persistent issuer replay. Standard families, global paid-check
  timeout ambiguity and the persistent minimum node retain their documented scope.
  See the [Storage handoff](../src/modules/peer_protocol/HANDOFF.md).
- [x] **10. [Storage module](9_Storage_module/PLAN.md).** Implement requester/provider store, ranged
  retrieval, renewal guards, removal, expiration and paid byte checks, with
  authorization, durable leases and hash verification. Keep local persistence
  independent of the paid Storage service. All 38 detailed steps are complete for the finite native Linux loopback
  profile; upstream issues [#10](https://github.com/beothorn/CR2SE/issues/10),
  [#11](https://github.com/beothorn/CR2SE/issues/11) and
  [#12](https://github.com/beothorn/CR2SE/issues/12) are resolved. The ordinary `serve` profile does not install
  paid Storage. See the [Storage handoff](../src/modules/storage/HANDOFF.md).
- [x] **11. [Minimum useful node milestone](10_Minimum_node/PLAN.md).** Compose the eight required modules
  with authorized local control and configured contacts. Demonstrate a paid
  nonempty store, restart and verified paid retrieval between two identities,
  then renewal/check/removal, finite credits, failures and concurrent spends.
  Run with every optional module absent; advertise only working capabilities.
  All 25 detailed steps are complete for the finite Linux minimum profile: two separately operated
  `serve --node-root` processes demonstrate AT-E01 and its failure cases. See the
  [node handoff](../src/composition/node/HANDOFF.md) and [acceptance evidence](../docs/minimum-node/ACCEPTANCE.md).
- [ ] **12. [Peer policy and admission](12_Peer_policy/PLAN.md).** Isolate every decision CR2SE leaves
  to local policy (admission, quotas, trust, bootstrap and earned credits,
  replica targets, check and repair cadence, connections) behind contracts with
  parameters in one validated file. Admit unconfigured peers, call clients over
  the connections they opened, keep server-hosted data at client replicas, and
  add a deterministic simulation harness for tuning. Planned in 23 steps; not started.

## TalkSphere-required modules and client journey

These modules are optional in CR2SE's minimum profile but required by
TalkSphere, whose clients (for example a messenger app) use messages, profile
pages and feeds through TalkSphere servers over CR2SE only.

- [ ] **13. [Messaging module](11_Messaging_module/PLAN.md).** Add signed/encrypted placement, recipient
  discovery/recovery, receipts, renewal, release/removal, status, checks and
  retries with durable replay guards. Planned in 38 steps; not started.
- [ ] **14. [Hosted pages and feeds in CR2SE](13_Hosted_publications/PLAN.md).** Specification work in
  CR2SE: let any provider host publisher-signed pages and feed posts, public or
  encrypted for chosen readers, reusing Messaging placements, so publishers
  that cannot accept connections (phones) can publish. Planned in 16 steps; not started.
- [ ] **15. Static Pages module.** Implement hosted pages and the existing
  publisher-served pages as accepted in milestone 14, reusing the Messaging
  placement implementation. Planned after milestone 14.
- [ ] **16. Public Feeds module.** Implement hosted feeds and the existing
  publisher-served feeds as accepted in milestone 14, with signed posts,
  attachments, cursors and local merging. Planned after milestone 14.
- [ ] **17. [Full client journey](14_Client_journey/PLAN.md).** Over CR2SE only, two servers and several
  outbound-only clients: clients earn credits by providing Storage, exchange
  encrypted messages, publish and read profile pages and feeds, and each server
  saves its hosted data at client replicas and retrieves it after loss.
  Planned in 17 steps; not started.

## Optional CR2SE modules and full-feature PoC

These modules are optional in CR2SE's minimum profile and not yet required by
TalkSphere. Implement each independently and register only complete capabilities.

- [ ] **18. Discovery module.** Add signed records, paid atomic indexing, bounded
  iterative lookup, cursors, retention checks and equivocation handling. Likely
  needed by the messenger to find people; reassess after the client journey.
- [ ] **19. Public File Sharing module.** Add canonical manifests, signed
  catalogs, paid ranges, resumable multi-host retrieval, safe paths and
  piece/file verification; reuse Discovery record formats through contracts.
- [ ] **20. Computation module.** Add an explicitly authorized, bounded runtime,
  exact metered pricing/reservations, timeout/error results and local checking.
- [ ] **21. Node API module.** Add authenticated local client access, versioned
  JSON-line messages, bounded blobs, session-scoped handles, cancellation and
  explicit spending/issuer authority. Test isolation and out-of-order replies.
- [ ] **22. Full-feature PoC and portability.** Complete the upstream feature
  matrix with three identities, independent encrypted replicas, retained keys
  and recovery catalog, recovery on replacement hardware with a provider
  offline, crash injection and uncertain-delivery tests. Cross-check canonical
  bytes with an independent codec/implementation. Publish reproducible build,
  run and restore instructions, version coverage, results and known limits.

Production readiness requires separate evaluation; completing a scaffold or a
minimum-profile demonstration does not complete the full-feature PoC.
