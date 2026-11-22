# Minimum useful node plan

Status: all 25 steps are complete for the finite Linux minimum profile; the full client journey and optional modules remain later milestones.

This is roadmap milestone 11 and planning folder 10, following Storage. It
composes the eight required modules into one `serve` process that a local
operator controls, so two separately operated identities can buy and sell paid
Storage over real connections.

## Steps

- [x] **1.** Reconcile the minimum-profile requirements with the completed module evidence.
- [x] **2.** Define the node state layout and validated node configuration.
- [x] **3.** Add explicit node initialization and inspection commands.
- [x] **4.** Move the native Storage clock and entropy adapters into production composition.
- [x] **5.** Persist a requester lease catalog in protected durable storage.
- [x] **6.** Load the provider Storage offering and capacity limits from operator configuration.
- [x] **7.** Open and recover every durable owner before admitting peer work.
- [x] **8.** Register Peer Protocol on the server's authenticated connections.
- [x] **9.** Install the Storage provider handler and publish its offering after recovery.
- [x] **10.** Install the Storage requester ports against the durable lease catalog.
- [x] **11.** Define the authorized local control channel and its bounded request format.
- [x] **12.** Add local commands to grant finite credits and set issuer trust.
- [x] **13.** Add a local command to fetch a contact's Board and Storage definition.
- [x] **14.** Add local commands to store and retrieve objects at a contact.
- [x] **15.** Add local commands to renew, check and remove a cataloged lease.
- [x] **16.** Add local commands to inspect balances, leases and uncertain outcomes.
- [x] **17.** Refuse requests for absent optional families before acceptance and without charge.
- [x] **18.** Drain accepted Storage work and local control requests on shutdown.
- [x] **19.** Document every new command and option in overview and detailed help.
- [x] **20.** Run AT-E01 between two separately operated node processes.
- [x] **21.** Exercise unauthorized access, stale expiration and concurrent reservations between nodes.
- [x] **22.** Exercise uncertain output delivery and provider restart between nodes.
- [x] **23.** Verify node logs and control replies exclude secrets and raw object content.
- [x] **24.** Complete the minimum-profile acceptance matrix.
- [x] **25.** Publish the minimum node handoff for the full client journey.

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
client journey (now milestone 13, after Messaging). A minimum node is not the full-feature PoC, and
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

Completed: Gap inventory recorded in [the acceptance document](../../docs/minimum-node/ACCEPTANCE.md); upstream rechecked at bc53df4.

### 2. Define the node state layout and validated node configuration

Define one node root with separate protected directories for Identity, Board,
Services, Ledger, Storage, the lease catalog and the control socket. Define an
effect-free validated configuration covering the Storage offering, capacity limits,
credit policy and control limits. Validation runs before any filesystem or network
effect. Test missing, duplicate, oversized and inconsistent values.

Completed: [Configuration](../../src/composition/node/config.md) and [layout](../../src/composition/node/layout.md) with unit tests; the finite profile bounds objects at 16384 bytes.

### 3. Add explicit node initialization and inspection commands

Add `node-init` to create a fresh node root explicitly and `node-show` to inspect
it without starting the network. Neither ever replaces existing state. Reuse the
existing identity store rules. Test fresh creation, refusal over existing state,
permission errors and inspection of a partly created root.

Completed: `node-init` and `node-show` ([initialization](../../src/composition/node/init.md)) with `tests/node_cli.rs`.

### 4. Move the native Storage clock and entropy adapters into production composition

Move the boot-identified uptime/UTC clock and the lease-ID entropy and hashing
ports out of the examples into `src/composition/`. They keep the documented
same-boot limitation and fail explicitly without trusted reboot evidence. Update
the examples to use them. Test clock failure, boot change and entropy failure
with injected platform readers.

Completed: [Adapters](../../src/composition/node/adapters.md) with an injectable platform reader; the Storage example uses them.

### 5. Persist a requester lease catalog in protected durable storage

Implement the Storage requester `Leases` port over a protected, atomically
updated catalog. Each record holds the lease ID, provider identity and contact,
content hash, size, expiry and optional encryption metadata. Records are written
only after full result validation and are scoped to their provider and requester.
Test reopen, corrupt or foreign records, concurrent updates and write failure.

Completed: [Catalog](../../src/composition/node/catalog.md): atomic, owner-only, provider- and requester-scoped.

### 6. Load the provider Storage offering and capacity limits from operator configuration

Build the complete standard Storage offering from configured prices, maxima and
retention, with the local identity as issuer, and validate it through the existing
terms checks. Configure finite capacity bytes, lease slots, pending operations and
retained results. Refuse unrepresentable prices or limits before publication.
Test every invalid term and a configuration change across restart.

Completed: [Offering](../../src/composition/node/offering.md) validated by the Board parser, Storage terms and standards catalogue.

### 7. Open and recover every durable owner before admitting peer work

Open Storage first and recover its participants, then open Board, Services and
Ledger under the same identity, following the Storage handoff order. Prepared
effects resolve from durable Services decisions before the listener accepts work.
A recovery failure stops startup with status 1 and a clear message. Test each
owner's open failure and recovery with pending prepared work.

Completed: [Owners](../../src/composition/node/owners.md) open in handoff order; a withdrawn offering discards only the derived Board snapshot.

### 8. Register Peer Protocol on the server's authenticated connections

Register the Peer Protocol endpoint with the server's Transport routing through
trusted composition, with bounded workers and configured limits. Board and
definition requests are served; unsupported protocols are still refused. Update
`serve` help, which currently says no application protocol is registered. Test
registration, limits and refusal of unknown protocols between real processes.

Completed: [Runtime](../../src/composition/node/runtime.md) registers Peer Protocol with the authenticated Transport; `serve --node-root` and its help are updated.

### 9. Install the Storage provider handler and publish its offering after recovery

Install the standard definition and provider handler through Services, then
publish the configured offering through Board only after step 7 succeeds. A node
with no offering configured publishes nothing and serves only as a requester.
Test publication order, withdrawn and changed terms across restart, and
that no provided offering appears before recovery.

Completed: The provider handler is installed only with a configured offering and the Board is published after recovery; withdrawn and changed terms are tested across restart.

### 10. Install the Storage requester ports against the durable lease catalog

Install the requester quote, result and check ports with the step 5 catalog and
the original provider and attempt expectations. Requester work uses explicit
total spending bounds and never chooses data or payment from peer input. Test
validation against catalog records and refusal for unknown leases.

Completed: Requester ports use a role-routed lease source: provider-side quotes read the provider journal, purchases read the catalog.

### 11. Define the authorized local control channel and its bounded request format

Create a Unix-domain socket in the owner-only node root and refuse to start if
its permissions are wider. Define bounded line-delimited JSON requests and replies
with exact integers, request IDs, deadlines and explicit errors. Limit concurrent
requests and line length. Test permissions, oversized and malformed lines,
unknown requests and client disconnects. This is not the CR2SE Node API.

Completed: [Control](../../src/composition/node/control/mod.md): owner-only socket, bounded lines and connections, busy while stopping.

### 12. Add local commands to grant finite credits and set issuer trust

Add commands that send grant and trust requests to the running node through the
control channel, bounded by explicit amounts. Grants are recorded once by request
ID so a resent command does not grant twice. Unsupported credit policies are
refused explicitly. Test exact balances, repeated requests and overflow refusal.

Completed: `grant`, `own` and `trust` ([commands](../../src/composition/node/control/commands.md)); grants are idempotent by request id while the node runs.

### 13. Add a local command to fetch a contact's Board and Storage definition

Fetch a configured contact's Board and Storage definition over Peer Protocol and
show the authenticated publisher, terms and prices. Test a missing offering, a
contact identity mismatch and an inconsistent definition.

Completed: `fetch` shows the authenticated publisher's provided offerings; the Storage definition is fetched only when offered.

### 14. Add local commands to store and retrieve objects at a contact

Store a local file at a contact within an explicit spending bound and catalog the
validated lease. Retrieve a full or partial range to a local file and verify the
whole-object hash for full retrievals. Bytes are written only after validation.
Test the exact `abc` hash, ranges, hash mismatch and a spending bound below the
quote.

Completed: `store` and `retrieve` validate through the Storage requester contracts; retrieved bytes are written only from a validated reply.

### 15. Add local commands to renew, check and remove a cataloged lease

Renew with the cataloged expected expiry, run a paid byte check with a private
expected value, and remove a lease. The catalog is updated only after each
validated result. Test a stale expected expiry, failed and inconclusive checks,
and repeated removal without a second charge.

Completed: `renew`, `check` and `remove` compute exact fixed prices because the provider requires them as the quote maximum.

### 16. Add local commands to inspect balances, leases and uncertain outcomes

Show issuer-separated balances, cataloged leases with expiry, and operations
whose outcome is unknown. Inspection never retries unknown work. Test output
for empty, active, expired and unknown states.

Completed: `status`, `leases`, `balance` and `outcomes`; unknown outcomes are journaled ([journal](../../src/composition/node/control/journal.md)).

### 17. Refuse requests for absent optional families before acceptance and without charge

Verify that requests for Computation, Messaging, Discovery, Files, Feeds and Pages
are refused as unsupported or missing before acceptance, with no reservation or
charge, and that the Board lists only Storage. Test each family name over a real
connection.

Completed: Only Storage is installed (`only_storage_is_installed`); `node-ctl` has no other family and the Board lists only Storage. Scope limit stated in the acceptance document.

### 18. Drain accepted Storage work and local control requests on shutdown

On SIGINT or SIGTERM stop new control and peer admission, let accepted Storage
work reach a durable outcome within `--shutdown-ms`, then close. A forced stop
leaves recoverable state. Test shutdown during an upload, a retrieval and a
pending control request, then restart.

Completed: Shutdown tests during an upload and a pending control request; see the [runtime](../../src/composition/node/runtime.md).

### 19. Document every new command and option in overview and detailed help

Add each new command and option to the overview and to `talksphere help <topic>`,
update the root README with two-terminal node instructions, and add CLI tests for
help, invalid arguments (status 2) and runtime failures (status 1).

Completed: Overview and detailed help for `node-init`, `node-show`, `node-ctl`, every option and `serve --node-root`; the [operator guide](../../docs/minimum-node/RUNBOOK.md) and root README are updated.

### 20. Run AT-E01 between two separately operated node processes

Start A and B as separate `serve` processes with only the eight required modules,
configured contacts and finite grants. A reads B's Board and definition, stores
`abc`, checks the expected hash and catalogs the lease. Restart B. A retrieves,
renews, checks with outcome pass and removes under current terms. Assert one
exact charge per paid operation, that the removed lease is unavailable and that
no optional offering is advertised.

Completed: `at_e01_minimum_useful_node_between_two_processes` in `tests/node_process.rs`.

### 21. Exercise unauthorized access, stale expiration and concurrent reservations between nodes

A third identity C cannot learn of or use A's lease. An operation after expiry is
refused without charge. Concurrent stores from A share finite credits and capacity
so that the accepted operations never exceed either limit. Assert exact balances
afterwards.

Completed: `unauthorized_stale_and_expired_requests` and `concurrent_stores_never_overspend_finite_credits`.

### 22. Exercise uncertain output delivery and provider restart between nodes

Cut the connection after B commits a store and before A receives the result. A
reports the outcome as unknown, keeps its hold and creates no replacement store
without a new request. Kill B at preparation, decision and acknowledgement and
show that each operation applies at most once after restart (AT-E04 at process
level).

Completed: `lost_store_result_is_unknown_and_never_replaced` and `provider_crash_after_commit_applies_once`; internal coordination cuts stay at library level.

### 23. Verify node logs and control replies exclude secrets and raw object content

Capture stderr and control replies from the acceptance runs and assert they
contain no private keys, tokens, object bytes or foreign lease metadata, and that
peer-controlled text is bounded. Logging stays on stderr and stdout carries only
intentional output.

Completed: `logs_and_replies_exclude_secrets_and_foreign_metadata` at trace level.

### 24. Complete the minimum-profile acceptance matrix

Map every PoC minimum-profile demonstration and the applicable AT-E01, AT-E04 and
AT-E05 assertions to executed tests in `docs/minimum-node/ACCEPTANCE.md`. Record
absent optional modules as not implemented, not passed. Run all AGENTS.md checks,
rustdoc with warnings denied and coverage above 80%, and audit links.

Completed: [Acceptance evidence](../../docs/minimum-node/ACCEPTANCE.md), with the check results recorded in the [handoff](../../src/composition/node/HANDOFF.md).

### 25. Publish the minimum node handoff for the full client journey

Write the supported platform, durability, limits, recovery and operator steps,
plus the contracts the client journey will build on (local control, lease catalog,
Ledger policy). Update the root README and roadmap. Keep the client journey and the
optional modules unchecked and do not expand their plans.

Completed: [Handoff](../../src/composition/node/HANDOFF.md); the root README and roadmap are updated.
