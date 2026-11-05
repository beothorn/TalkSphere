# Transport integration plan

Status: execution started; steps 1–12 are complete.

This is roadmap milestone 5 and planning folder 4, following the completed
Transport, Cryptography and Identity plans. It composes those modules into an
executable authenticated Transport node. Implementation was subsequently authorized by the user.

## Steps

- [x] **1.** Reconcile the existing Transport evidence with the reviewed upstream contracts.
- [x] **2.** Define the server lifecycle and validated configuration contracts.
- [x] **3.** Add deterministic runtime helpers and isolated process fixtures.
- [x] **4.** Add explicit local identity initialization and inspection commands.
- [x] **5.** Parse server options and document every option in detailed help.
- [x] **6.** Configure structured operational logging at application startup.
- [x] **7.** Load one protected identity and grant the server scoped authority.
- [x] **8.** Compose the real Transport driver and platform adapters.
- [x] **9.** Drive Transport events with a wakeable runtime loop.
- [x] **10.** Start the configured listener and report its actual bound address.
- [x] **11.** Connect configured contacts while preserving their expected identities.
- [x] **12.** Route authenticated events to authorized local consumers.
- [ ] **13.** Drain connections and release resources on shutdown.
- [ ] **14.** Enable the serve command through the composed runtime.
- [ ] **15.** Authenticate two independent server processes in both connection directions.
- [ ] **16.** Verify protected framing across fragmented and coalesced socket traffic.
- [ ] **17.** Verify real handshake failures before session key publication.
- [ ] **18.** Reject tampering and replay before application dispatch.
- [ ] **19.** Verify stream allocation and protocol selection on authenticated connections.
- [ ] **20.** Verify multiplexing, half-close and cancellation across real peers.
- [ ] **21.** Verify probes and graceful draining across real peers.
- [ ] **22.** Verify truncation, resource bounds and slow-consumer isolation.
- [ ] **23.** Verify restart, startup failures and diagnostic privacy.
- [ ] **24.** Audit Transport acceptance and publish the server integration handoff.

## Reference baseline and scope

Planning uses the existing Identity baseline
[`d40b6278f24430e814d7fa82094f57261bfcac9f`](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f),
not a claim that it is the latest upstream revision. The Transport acceptance
suite at that revision was re-read for this plan. Before implementation, compare
upstream changes and record the adopted revision and affected fixtures.

Read the upstream [Transport specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/transport/README.md),
[input contract](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/transport/inputs/README.md),
[output contract](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/transport/outputs/README.md),
[pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/transport/pseudocode/README.md),
[vectors](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/transport/pseudocode/TestVectors.md),
[TR-01–TR-15](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/transport/acceptanceTests/README.md),
[ModuleContracts](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md)
and [shared acceptance cases](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/AcceptanceTests.md).
Review the Identity and Cryptography contracts when wiring their capabilities.

Local starting points are the [driver](../../src/modules/transport/driver/mod.md),
[platform adapters](../../src/modules/transport/adapters/README.md),
[Identity adapter](../../src/composition/identity.md),
[Cryptography adapter](../../src/composition/cryptography.md), and the
[Transport](../../src/modules/transport/ACCEPTANCE.md) and
[Identity](../../src/modules/identity/ACCEPTANCE.md) evidence matrices.
Existing real handshake tests operate below the full server runtime; existing
TCP tests and scripted driver tests must not be mistaken for process-level evidence.

Keep orchestration under `src/composition/` with focused runtime/configuration/
lifecycle files as needed, CLI behavior under `src/cli/`, and protocol fixes in
their owning modules. Keep main thin. Reuse the existing Mio readiness worker,
SystemClock, Driver and capability adapters before adding an executor dependency.
Linux is the initial supported protected-storage platform.

This milestone does not implement Board, Ledger, Services, Peer Protocol, paid
Storage, Discovery or Node API. Test consumers may register an isolated test
protocol through trusted composition; never advertise a fake `cr2se.peer` service
or add a production arbitrary-payload echo/debug interface. The production node
can authenticate connections and reject unsupported application protocols.
Private keys are not CLI arguments or peer data. Existing identity state is never
silently replaced. No minimum-useful-node or independent interoperability claim
follows merely from two TalkSphere processes communicating.

## Execution rules

Follow [AGENTS.md](../../AGENTS.md) for each step: unit tests and relevant failure
cases, integration tests at real boundaries, entry tracing, companion contract
Markdown, current README/help and production coverage above 80%. Keep each step
as a focused commit with a descriptive title and body explaining intent and
verification; record evidence before checking it off. Final audits consolidate
checks rather than defer testing. Do not enlarge later roadmap milestones.

Search open and closed upstream issues before reporting confirmed protocol defects;
file new language-independent findings on GitHub and retain their local evidence
and issue URLs under `CR2SE bugs/`. Runtime design choices and Rust tooling bugs
are local implementation work. No new confirmed protocol defect arose in this
planning review.

## Details

### 1. Baseline and acceptance inventory

Compare current upstream against the recorded revisions, read all referenced
contracts and trace existing evidence to actual tests. Resolve stale pending notes
using the later Identity/Cryptography supplements. Record a TR-01–TR-15 and
AT-C01–AT-C04 gap inventory, including authentication-before-key-derivation order,
weak-point negatives and diagnostic vectors. Preserve earlier fixture provenance;
never silently replace expected wire bytes to match implementation output.

Completed: compared upstream main on 2026-11-05, retained d40b6278 because no
protocol files changed, re-read the upstream contracts and recorded the
[acceptance gap inventory](../../docs/transport-integration/ACCEPTANCE.md).

### 2. Runtime configuration and lifecycle

Define validated options, startup stages, ready/stopping/stopped outcomes and typed
failures. Specify finite connection/stream/queue limits, timeouts, listener endpoint,
identity-store path and explicit contact sources. Use loopback as the documented
default bind policy; allow port zero for tests and report the assigned address.
Distinguish configured budgets from protocol maxima. Test invalid combinations and
ensure validation precedes filesystem changes and network effects.

Completed: added effect-free validated server options, finite defaults, contact
validation, lifecycle stages and typed failure contracts with companion docs and
unit tests. Required formatting, Clippy, locked tests/build and help checks pass;
production coverage is 4812/4999 = 96.26%.

### 3. Runtime and process fixtures

Provide controllable shutdown, event sinks, wakeups and clock substitutes for
runtime tests; use real cryptography whenever authentication is asserted. Build
fresh-process helpers with owner-only temporary stores, OS-assigned ports, bounded
readiness waits, captured outputs and guaranteed child cleanup. Avoid fixed sleeps
and shared ports. Separate deterministic wire fixtures from production CSPRNG use.

Completed: added test-only deterministic wake/shutdown probes and isolated child
fixtures with bounded stdout/stderr records, readiness waits and owned cleanup.
Self-checks execute real CLI children; existing ManualClock/socket substitutes are
reused. All required checks pass; production coverage excludes the helpers and is
4812/4999 = 96.26%.

### 4. Explicit identity administration

Add narrowly scoped local initialization and public inspection commands using the
existing Identity provider. Select a consistent command spelling and store option;
creation is explicit and refuses existing state, while inspection never creates it.
Expose only public IDs/credentials as intentional output. Test duplicate creation,
denied/insecure stores, missing/corrupt records and unsupported platforms. Document
protection, storage location and usage; keep backup/import CLI out of this milestone.

Completed: Added explicit identity-init/identity-show with required store selection and discoverable command/option help. Parser unit and real executable tests cover creation, inspection, duplicate creation, missing and corrupt state without replacement. Required checks pass; production coverage: 4843/5032 = 96.24%.

### 5. Server CLI and discoverability

Add options for the identity store, listening endpoint, configured contacts and
necessary finite runtime limits, choosing exact spellings once and documenting
defaults, units, repetition and conflicts. Contacts use Identity's canonical payload
parser; define whether explicit options or a stored snapshot supplies contacts and
reject ambiguous merging. ID-only contacts require an explicit resolution error
because Discovery is absent. Every new command/option needs `help <topic>` including
option spellings, parser tests and README examples. Syntax errors exit 2 with no I/O.

Completed: Added bounded exact server option parsing, validated finite budgets and endpoint-bearing contacts, mutually exclusive stored/explicit contact selection, and per-spelling detailed help. Parser unit and existing CLI tests pass with required checks; help --listen exercised. Production coverage: 4906/5098 = 96.23%.

### 6. Application logging

Install a shared structured tracing subscriber at composition startup with target
and level filtering, a documented default, and stderr output. Keep stdout for
intentional command results. Bound peer-controlled diagnostic text and repetitive
failures; exclude private stores, keys and payload contents. Test filters, output
streams, malformed filter handling and discoverability of any logging option.

Completed: Installed shared tracing-subscriber stderr logging with validated RUST_LOG target/level filters and info default; help/version skip logging setup. Unit and executable tests cover filters, malformed configuration and stdout discipline; help logging exercised. All required checks pass; production coverage: 4933/5126 = 96.23%.

### 7. Protected identity startup

Load exactly one configured identity under its exclusive store lock, validate its
complete association and issue only needed read/sign grants. Denied, absent or
corrupt credentials fail before binding sockets; startup never auto-generates keys.
Retain provider ownership throughout runtime. Test concurrent writers, wrong grants,
unsupported platforms and cleanup after partial startup.

Completed: Load existing Linux protected credentials before network construction, retain the exclusive lock, grant read/sign only and validate explicitly selected stored contacts. Unit and real executable-assisted integration checks cover missing/corrupt state, public identity projection and lock release. Required checks pass; production coverage: 4970/5164 = 96.24%.

### 8. Real dependency composition

Construct TransportIdentity, TransportCrypto with SystemEntropy, TcpProvider and
SystemClock, then the configured Driver through public contracts. Check how the
clock and socket adapters share the readiness worker. No test-only substitute may
be selected from CLI configuration. Test constructor failure cleanup, capability
lifetimes and that construction itself opens no listener.

Completed: Composed real SystemEntropy Cryptography, protected scoped Identity, TcpProvider and its shared SystemClock into Driver. Integration evidence checks zero initial socket occupancy and retention/release of the identity lock; no listener is bound by construction. All required checks pass; production coverage: 4990/5184 = 96.26%.

### 9. Wakeable event loop

Poll Driver events until Pending, then wait on a race-safe wake mechanism that also
handles deadlines and shutdown. Preserve self-wakes and wake-before-wait delivery;
avoid spinning or sleeping through available work. Bound work per iteration so
shutdown and other connections remain responsive. Unit-test idle waiting, spurious
wakeups, queued events, timer wakeups and worker failure handling.

Completed: Added bounded 64-event polling with persistent task notifications, condition-variable waiting and injected local shutdown. Tests cover wake-before-wait, repeated notification, shutdown, pending/full turns, consumer failure and unexpected driver termination. All required checks pass; production coverage: 5045/5242 = 96.24%.

### 10. Listening and readiness

Submit Listen, consume its result and report readiness only after the real bind
succeeds and startup prerequisites are satisfied. Report actual bound IP/port and
public local identity through a documented bounded status output usable by process
tests. Test bind conflicts, invalid endpoints, readiness timing, listener admission
limits and cleanup when startup fails after acquiring resources.

Completed: Added one-time listener startup with Ready emitted only after successful bind and the actual assigned address/public ID. Real socket tests cover port zero, duplicate startup, bind conflict and port release after drop. Required checks pass; production coverage: 5066/5264 = 96.24%.

### 11. Configured outbound contacts

Translate validated contact snapshots into ConnectRequests with ExpectedPeer::Known.
Preserve the binary expected ID even if an endpoint changes. Bound simultaneous
attempts and expose per-contact outcomes. Use explicit one-shot startup attempts
initially; do not invent an unbounded reconnect/retry policy. Test unreachable peers,
timeouts, duplicate input policy, endpoint mismatch and continued listener operation.

Completed: Added bounded one-shot contact startup preserving ExpectedPeer::Known and per-contact admission outcomes. Real composition checks retained ID/authority, readiness ordering and rejection of repeated startup; the listener remains independent of attempts. Required checks pass; production coverage: 5101/5300 = 96.25%.

### 12. Event routing and protocol ownership

Route authenticated connection and stream events through registered local owners;
consume terminal outcomes so resources are released. No consumer receives provisional
handshake data. Register test protocols only in test composition and exercise real
stream exchange there; production unsupported protocols follow Transport errors.
Test owner isolation, callback/consumer failure, bounded event handling and unknown
protocol rejection without introducing Peer Protocol semantics.

Completed: Added bounded trusted consumer registration and stream routing by owner capability; non-stream observations return to composition. Foreign owner events are rejected before delivery and failed consumers request stream cancellation without retries. Production starts with no fake application protocols. Required checks pass; production coverage: 5140/5344 = 96.18%.

### 13. Controlled shutdown

Translate process termination into runtime shutdown through an injected platform
signal boundary. Stop accepting/initiating work, request GOAWAY/drain, allow accepted
streams to finish within a finite deadline, then close remaining connections and
release keys, locks and readiness resources. Define repeated signals and force-close
behavior. Test idle/busy shutdown, deadline expiry and idempotent cleanup; process
signal handlers must not perform blocking protocol operations.

### 14. Executable server activation

Wire `serve` to the validated runtime only after startup, polling and shutdown are
usable. Update the unavailable-runtime diagnostic and help to reflect actual support.
Runtime failures exit 1 with secret-free stderr explanations; normal shutdown exits
successfully. Test executable startup and termination, no-argument help, invalid
arguments and all new detailed help topics. Retain explicit errors for unavailable
platforms and capabilities.

### 15. Independent two-process authentication

Create separate stores and start two real processes with CSPRNG-generated credentials,
then connect A to B and B to A in separate sessions. Observe exact authenticated
peer IDs and absence of application dispatch before verification. Restart with the
same keys and verify stable identity plus fresh sessions. Use the real driver with
a test-owned consumer for payload evidence; keep that harness out of the production
CLI. Distinguish process tests from fixed-vector checks below.

### 16. Framing and fixed-vector integration

Reuse literal HELLO/AUTH/first-tag vectors with real Identity/Cryptography in the
composed driver harness. Control byte delivery and short writes through test socket
wrappers, since TCP does not guarantee read segmentation. Cover every split,
one-byte chunks, coalesced AUTH plus protected frames, serialized partial writes,
DATA lengths 1/65536 and oversized/invalid headers. Repeat representative scenarios
through real loopback sockets; retain exact-byte assertions for TR-01–TR-03/05.

### 17. Handshake rejection and derivation order

Close the TR-04/05 matrix for both roles: ordering/duplicates, widths/selectors,
wrong expected ID, invalid AUTH, weak keys, invalid R/S and all-zero agreement.
Use an observing wrapper around real cryptography to prove successful peer AUTH
and complete local AUTH write precede shared-secret/HKDF work. Invalid AUTH must
win over a zero ephemeral point. Test withheld AUTH, partial local AUTH write failure,
timeouts and same-identity peers with independently fresh ephemeral material.

### 18. Integrity and replay

Exercise valid real tags and mutations of headers, payloads and tags, next-sequence
replay, swapped direction material and old-session tags. Assert silent close, no
unauthenticated dispatch and no sequence advance on incomplete/invalid frames.
Use controlled state injection for uint64 exhaustion instead of enormous traffic;
keep it test-only. Observe ephemeral/session secret release on every applicable
exit, retaining the documented limitations of memory-erasure evidence.

### 19. Stream IDs and OPEN

Run TR-07/08 through authenticated drivers: exact parity/sequence, consumed refused
IDs, reuse/skips and never-created references. Exercise valid registration, unknown
protocol/version and malformed OPEN independently of integrity by signing malformed
fixtures with correct real tags. Controlled uint32 exhaustion must produce the
specified GOAWAY without wrap. Keep application protocol handlers test-owned.

### 20. Multiplexing and terminal behavior

Run TR-09/10 with interleaved streams in both directions, both half-close orders,
DATA/repeated END violations, cancellation, queued terminal traffic and independent
stream progress. Assert exact per-direction bytes and ownership, no whole-connection
failure for stream-local errors, and no hidden application retry or Ledger action.

### 21. Probes and graceful drain

Exercise TR-11/12: matching eight-byte PONG, unsolicited/duplicate/unrelated responses,
probe expiry, GOAWAY bounds, increasing repeated bounds, late OPEN refusal and
accepted-stream completion. Combine deterministic timer tests with real socket
shutdown and confirm completed results remain complete after connection closure.

### 22. Truncation and bounded operation

Complete TR-13–TR-15 with correctly tagged malformed controls and exact diagnostic
vectors, EOF at every representative protected-frame prefix, unfinished versus
completed streams and no resynchronization. Exercise handshake deadlines, listener/
connection/stream/byte caps, slow consumers and fairness under bounded concurrent
load. Assert resource refusal is never normal service completion; use synthetic
state for impractically large limits and explicit counters for occupancy claims.

### 23. Restart and operational failures

Test process restart with retained credentials/contacts and fresh sessions, bind and
entropy failures, lost readiness workers and termination during startup or active
traffic. Confirm locks/sockets are released and durable identity is never replaced.
Capture structured diagnostics at TRACE across success/failure; assert no secrets,
private records or payloads and bounded handling of malicious diagnostic strings.
Document that Transport restart fails in-flight work rather than resuming service
transactions or replaying application requests.

### 24. Acceptance and handoff

Replace stale pending evidence with an assertion-level matrix for TR-01–TR-15 and
applicable AT-C01–AT-C04, identifying real primitives, deterministic wrappers, real
TCP and separate-process coverage. Resolve implementation gaps before completion;
report protocol blockers honestly with upstream issue links. Keep independent
interoperability, minimum-profile node and paid recovery outside this claim.

Publish reproducible initialization, inspection, two-node start/connect and shutdown
commands with Linux protection, defaults, limits and unsupported capabilities.
Update root/module READMEs, companion contracts, overview/detailed help and roadmap.
Run AGENTS.md checks, affected commands, API docs and production coverage above 80%;
record measured evidence. Only then mark milestone 5 complete and hand off to Board
without expanding or implementing its plan in this task.
