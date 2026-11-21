# Minimum useful node plan

Status: planned; the control-socket design is accepted, but no step is authorized or implemented yet.

This is roadmap milestone 11 and planning folder 10, following Storage. It
composes the eight required modules into one `serve` process that a local
operator controls, so two separately operated identities can buy and sell paid
Storage over real connections.

## Steps

- [ ] **1.** Reconcile the minimum-profile requirements with the completed module evidence.
- [ ] **2.** Define the node state layout and validated node configuration.
- [ ] **3.** Add explicit node initialization and inspection commands.
- [ ] **4.** Move the native Storage clock and entropy adapters into production composition.
- [ ] **5.** Persist a requester lease catalog in protected durable storage.
- [ ] **6.** Load the provider Storage offering and capacity limits from operator configuration.
- [ ] **7.** Open and recover every durable owner before admitting peer work.
- [ ] **8.** Register Peer Protocol on the server's authenticated connections.
- [ ] **9.** Install the Storage provider handler and publish its offering after recovery.
- [ ] **10.** Install the Storage requester ports against the durable lease catalog.
- [ ] **11.** Define the authorized local control channel and its bounded request format.
- [ ] **12.** Add local commands to grant finite credits and set issuer trust.
- [ ] **13.** Add a local command to fetch a contact's Board and Storage definition.
- [ ] **14.** Add local commands to store and retrieve objects at a contact.
- [ ] **15.** Add local commands to renew, check and remove a cataloged lease.
- [ ] **16.** Add local commands to inspect balances, leases and uncertain outcomes.
- [ ] **17.** Refuse requests for absent optional families before acceptance and without charge.
- [ ] **18.** Drain accepted Storage work and local control requests on shutdown.
- [ ] **19.** Document every new command and option in overview and detailed help.
- [ ] **20.** Run AT-E01 between two separately operated node processes.
- [ ] **21.** Exercise unauthorized access, stale expiration and concurrent reservations between nodes.
- [ ] **22.** Exercise uncertain output delivery and provider restart between nodes.
- [ ] **23.** Verify node logs and control replies exclude secrets and raw object content.
- [ ] **24.** Complete the minimum-profile acceptance matrix.
- [ ] **25.** Publish the minimum node handoff for the full client journey.

## Baseline, scope and review

Planned on 2026-11-21. Upstream main is
[CR2SE bc53df4](https://github.com/beothorn/CR2SE/tree/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16),
which also contains the Storage behavior adopted for the completed module.
Recheck it before step 1 and record any newer change.

- [First proof of concept, minimum useful profile](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/PoC.md#minimum-useful-profile).
- [Shared acceptance cases AT-E01–AT-E08](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/AcceptanceTests.md#end-to-end-acceptance).
- [Required and optional capabilities](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/ModuleContracts.md#required-and-optional-capabilities).

Local starting points are the [Storage handoff](../../src/modules/storage/HANDOFF.md),
the [Peer Protocol handoff](../../src/modules/peer_protocol/HANDOFF.md), the
[server composition guide](../../src/composition/server/README.md) and the
Storage example owners in `examples/storage/`. The examples are evidence of the
composition, not production code to copy unchanged: their offering, clock,
catalog and balances are demonstration-only.

In scope: the eight required modules composed in `serve`, operator configuration,
authorized local control, configured contacts, finite grants and trust, and
two-process acceptance on Linux. Out of scope: the CR2SE Node API, Discovery,
the six optional families, wanted Storage through local control, and the full
client journey (milestone 12). A minimum node is not the full-feature PoC, and
two TalkSphere processes do not establish interoperability with another
implementation.

Design choice, accepted on 2026-11-21: the running `serve` process holds each identity's
single-writer lock, so a separate command cannot open the same owners. The plan
therefore adds a local control channel: a Unix-domain socket inside the
owner-only node directory, with bounded line-delimited requests. Filesystem
permissions are its authority. It is a TalkSphere operator interface, not the
CR2SE Node API, and it is never reachable from the network. The alternative,
one-shot client processes that briefly own the identity, cannot serve and buy
at the same time and is not planned.

## Execution rules

Follow [AGENTS.md](../../AGENTS.md) for each step: unit tests with failure and
boundary cases, integration tests at real process boundaries, entry tracing,
companion contract Markdown, current README and help, and production coverage
above 80% measured with [the coverage procedure](../../docs/coverage.md). Keep
composition under `src/composition/`, CLI behavior under `src/cli/` and protocol
fixes in their owning modules. Commit each verified step separately with a
descriptive title and body. Check a step only after its tests and docs pass.

Search open and closed upstream issues before reporting a confirmed protocol
defect, file new language-independent findings on GitHub and keep the local
record under [CR2SE bugs](../../CR2SE%20bugs/README.md). No new protocol defect is
asserted by this plan.

## Details

### 1. Reconcile the minimum-profile requirements with the completed module evidence

Map each minimum-profile demonstration in PoC.md and the AT-E01, AT-E04 and
AT-E05 assertions to existing module evidence, listing what only a composed node
can prove. Recheck upstream main. Record the gap inventory in a new
`docs/minimum-node/ACCEPTANCE.md`. No code changes.

### 2. Define the node state layout and validated node configuration

Define one node root with separate protected directories for Identity, Board,
Services, Ledger, Storage, the lease catalog and the control socket. Define an
effect-free validated configuration covering the Storage offering, capacity limits,
credit policy and control limits. Validation runs before any filesystem or network
effect. Test missing, duplicate, oversized and inconsistent values.

### 3. Add explicit node initialization and inspection commands

Add `node-init` to create a fresh node root explicitly and `node-show` to inspect
it without starting the network. Neither ever replaces existing state. Reuse the
existing identity store rules. Test fresh creation, refusal over existing state,
permission errors and inspection of a partly created root.

### 4. Move the native Storage clock and entropy adapters into production composition

Move the boot-identified uptime/UTC clock and the lease-ID entropy and hashing
ports out of the examples into `src/composition/`. They keep the documented
same-boot limitation and fail explicitly without trusted reboot evidence. Update
the examples to use them. Test clock failure, boot change and entropy failure
with injected platform readers.

### 5. Persist a requester lease catalog in protected durable storage

Implement the Storage requester `Leases` port over a protected, atomically
updated catalog. Each record holds the lease ID, provider identity and contact,
content hash, size, expiry and optional encryption metadata. Records are written
only after full result validation and are scoped to their provider and requester.
Test reopen, corrupt or foreign records, concurrent updates and write failure.

### 6. Load the provider Storage offering and capacity limits from operator configuration

Build the complete standard Storage offering from configured prices, maxima and
retention, with the local identity as issuer, and validate it through the existing
terms checks. Configure finite capacity bytes, lease slots, pending operations and
retained results. Refuse unrepresentable prices or limits before publication.
Test every invalid term and a configuration change across restart.

### 7. Open and recover every durable owner before admitting peer work

Open Storage first and recover its participants, then open Board, Services and
Ledger under the same identity, following the Storage handoff order. Prepared
effects resolve from durable Services decisions before the listener accepts work.
A recovery failure stops startup with status 1 and a clear message. Test each
owner's open failure and recovery with pending prepared work.

### 8. Register Peer Protocol on the server's authenticated connections

Register the Peer Protocol endpoint with the server's Transport routing through
trusted composition, with bounded workers and configured limits. Board and
definition requests are served; unsupported protocols are still refused. Update
`serve` help, which currently says no application protocol is registered. Test
registration, limits and refusal of unknown protocols between real processes.

### 9. Install the Storage provider handler and publish its offering after recovery

Install the standard definition and provider handler through Services, then
publish the configured offering through Board only after step 7 succeeds. A node
with no offering configured publishes nothing and serves only as a requester.
Test publication order, withdrawn and changed terms across restart, and
that no provided offering appears before recovery.

### 10. Install the Storage requester ports against the durable lease catalog

Install the requester quote, result and check ports with the step 5 catalog and
the original provider and attempt expectations. Requester work uses explicit
total spending bounds and never chooses data or payment from peer input. Test
validation against catalog records and refusal for unknown leases.

### 11. Define the authorized local control channel and its bounded request format

Create a Unix-domain socket in the owner-only node root and refuse to start if
its permissions are wider. Define bounded line-delimited JSON requests and replies
with exact integers, request IDs, deadlines and explicit errors. Limit concurrent
requests and line length. Test permissions, oversized and malformed lines,
unknown requests and client disconnects. This is not the CR2SE Node API.

### 12. Add local commands to grant finite credits and set issuer trust

Add commands that send grant and trust requests to the running node through the
control channel, bounded by explicit amounts. Grants are recorded once by request
ID so a resent command does not grant twice. Unsupported credit policies are
refused explicitly. Test exact balances, repeated requests and overflow refusal.

### 13. Add a local command to fetch a contact's Board and Storage definition

Fetch a configured contact's Board and Storage definition over Peer Protocol and
show the authenticated publisher, terms and prices. Test a missing offering, a
contact identity mismatch and an inconsistent definition.

### 14. Add local commands to store and retrieve objects at a contact

Store a local file at a contact within an explicit spending bound and catalog the
validated lease. Retrieve a full or partial range to a local file and verify the
whole-object hash for full retrievals. Bytes are written only after validation.
Test the exact `abc` hash, ranges, hash mismatch and a spending bound below the
quote.

### 15. Add local commands to renew, check and remove a cataloged lease

Renew with the cataloged expected expiry, run a paid byte check with a private
expected value, and remove a lease. The catalog is updated only after each
validated result. Test a stale expected expiry, failed and inconclusive checks,
and repeated removal without a second charge.

### 16. Add local commands to inspect balances, leases and uncertain outcomes

Show issuer-separated balances, cataloged leases with expiry, and operations
whose outcome is unknown. Inspection never retries unknown work. Test output
for empty, active, expired and unknown states.

### 17. Refuse requests for absent optional families before acceptance and without charge

Verify that requests for Computation, Messaging, Discovery, Files, Feeds and Pages
are refused as unsupported or missing before acceptance, with no reservation or
charge, and that the Board lists only Storage. Test each family name over a real
connection.

### 18. Drain accepted Storage work and local control requests on shutdown

On SIGINT or SIGTERM stop new control and peer admission, let accepted Storage
work reach a durable outcome within `--shutdown-ms`, then close. A forced stop
leaves recoverable state. Test shutdown during an upload, a retrieval and a
pending control request, then restart.

### 19. Document every new command and option in overview and detailed help

Add each new command and option to the overview and to `talksphere help <topic>`,
update the root README with two-terminal node instructions, and add CLI tests for
help, invalid arguments (status 2) and runtime failures (status 1).

### 20. Run AT-E01 between two separately operated node processes

Start A and B as separate `serve` processes with only the eight required modules,
configured contacts and finite grants. A reads B's Board and definition, stores
`abc`, checks the expected hash and catalogs the lease. Restart B. A retrieves,
renews, checks with outcome pass and removes under current terms. Assert one
exact charge per paid operation, that the removed lease is unavailable and that
no optional offering is advertised.

### 21. Exercise unauthorized access, stale expiration and concurrent reservations between nodes

A third identity C cannot learn of or use A's lease. An operation after expiry is
refused without charge. Concurrent stores from A share finite credits and capacity
so that the accepted operations never exceed either limit. Assert exact balances
afterwards.

### 22. Exercise uncertain output delivery and provider restart between nodes

Cut the connection after B commits a store and before A receives the result. A
reports the outcome as unknown, keeps its hold and creates no replacement store
without a new request. Kill B at preparation, decision and acknowledgement and
show that each operation applies at most once after restart (AT-E04 at process
level).

### 23. Verify node logs and control replies exclude secrets and raw object content

Capture stderr and control replies from the acceptance runs and assert they
contain no private keys, tokens, object bytes or foreign lease metadata, and that
peer-controlled text is bounded. Logging stays on stderr and stdout carries only
intentional output.

### 24. Complete the minimum-profile acceptance matrix

Map every PoC minimum-profile demonstration and the applicable AT-E01, AT-E04 and
AT-E05 assertions to executed tests in `docs/minimum-node/ACCEPTANCE.md`. Record
absent optional modules as not implemented, not passed. Run all AGENTS.md checks,
rustdoc with warnings denied and coverage above 80%, and audit links.

### 25. Publish the minimum node handoff for the full client journey

Write the supported platform, durability, limits, recovery and operator steps,
plus the contracts milestone 12 will build on (local control, lease catalog,
Ledger policy). Update the root README and roadmap. Keep milestone 12 and the
optional modules unchecked and do not expand their plans.
