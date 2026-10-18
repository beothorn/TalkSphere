# Transport module plan

Status: steps 1–22 are implemented and checked; steps 23–24 have not been started.

This breaks down Transport, the first unfinished module in the
[main roadmap](../PLAN.md). Its folder is numbered 1; it remains roadmap step 2
because project setup is already complete.

The steps below build Transport with replaceable dependencies and test-only
Identity/Cryptography substitutes. Real cryptographic verification and the
authenticated two-node demonstration remain in roadmap steps 3–5. Completing
these steps alone does not establish protocol conformance or enable `serve`
with simulated authentication.

Every implementation step includes unit tests for its behavior and failure
paths, plus updates to any affected documentation or help. The last verification
step brings those tests together; it does not defer testing until the end.

## Steps

- [x] **1.** Define Transport's public interface and replaceable dependencies.
- [x] **2.** Add deterministic test helpers for Transport's dependencies.
- [x] **3.** Implement frame header encoding and validation.
- [x] **4.** Implement connection-control and error payload formats.
- [x] **5.** Read and write complete frames across partial socket operations.
- [x] **6.** Implement HELLO and AUTH handshake payload formats.
- [x] **7.** Construct authentication transcripts and directional key derivation requests.
- [x] **8.** Implement handshake sequencing and peer identity checks.
- [x] **9.** Add frame integrity verification and replay protection.
- [x] **10.** Implement stream-opening payload encoding and validation.
- [x] **11.** Allocate and validate stream identifiers in the required order.
- [x] **12.** Route new streams to registered protocol handlers.
- [x] **13.** Exchange ordered data across multiple bidirectional streams.
- [x] **14.** Support independent stream half-closing and normal completion.
- [x] **15.** Implement stream cancellation and terminal-state handling.
- [x] **16.** Apply the required stream-level and connection-level error responses.
- [x] **17.** Add ping requests with response matching and timeouts.
- [x] **18.** Implement graceful connection shutdown with GOAWAY.
- [x] **19.** Enforce handshake deadlines and release connection resources on closure.
- [x] **20.** Enforce limits on connections, streams and queued bytes.
- [x] **21.** Schedule active streams fairly when consumers or sockets are slow.
- [x] **22.** Add TCP adapters for outgoing and incoming connections.
- [ ] **23.** Verify Transport behavior against the upstream acceptance cases.
- [ ] **24.** Document completed coverage and the remaining real-cryptography integration.

## Reference baseline

Use CR2SE revision
[`8e848eb2e70ef7cd4490a753f9e647fdddcd99fd`](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd)
as the starting reference, checking for upstream changes before implementation.

- [Transport specification][spec] and [pseudocode][pseudocode].
- [Input contract][inputs], [output contract][outputs] and [module contracts][contracts].
- [Acceptance cases TR-01 through TR-15][acceptance] and [fixed wire vectors][vectors].

The wire specification is authoritative; pseudocode names do not dictate Rust
types or concurrency choices. Record confirmed language-independent defects
under [CR2SE bugs](../../CR2SE%20bugs/README.md), with evidence and revision links.

## Step details

### 1. Public interface and dependencies

Define explicit requests/events for connection control, protocol registration,
stream data and terminal outcomes, with scoped connection/stream handles and
machine-readable failures. Define replaceable socket, monotonic clock,
Identity and Cryptography interfaces using the upstream input/output contracts.
Keep identity authority and stream ownership separate from peer-provided data.

Keep code under `src/modules/transport/`, adding focused files or folders such
as `codec/`, `handshake/`, `stream/` and `adapters/` only when needed. Document
the selected I/O/concurrency approach without making it part of the protocol.
Test invalid local configuration and handle ownership rules as they are added;
no Board, Ledger or service implementation is required.

Completed: [public contracts and dependency ports](../../src/modules/transport/README.md)
define requests/events, typed failures, opaque scoped handles, validated local
policy and the polling-based I/O/concurrency boundary. Unit tests cover invalid
configuration, expected contacts and cross-owner/cross-connection access;
documentation tests check usable configuration and reject forged capabilities.
Formatting, Clippy with warnings denied, all 30 tests, build, API documentation
and the overview/detailed help checks passed. No driver, adapter, protocol codec
or deterministic dependency harness was implemented in this step.

### 2. Deterministic test helpers

Provide test-owned sockets that split reads/writes, combine incoming frames,
fail writes and end at chosen byte offsets, plus a manually advanced clock and
a recording protocol consumer. Add scripted Identity/Cryptography substitutes
that record exact inputs and can reject signatures, shared secrets or tags.

Keep substitutes available only to tests; do not expose a runtime switch for
fake authentication. Test the helper behaviors used by assertions and retain
upstream fixtures with their revision and expected bytes. Scripted results can
verify orchestration but cannot prove cryptographic correctness.

Completed: [test-only dependency helpers](../../src/modules/transport/test_support/README.md)
provide scripted sockets, connect/listen/accept operations, a manual clock,
an event recorder and strict Identity/Cryptography call expectations. Tests
cover fragmentation, coalescing, every frame truncation offset, short/failed
writes, pending wakeups, resource release, consumer backpressure and dependency
failures. The unchanged upstream vector snapshot retains the reference revision;
tests verify its structure without claiming cryptographic correctness.
Formatting, Clippy with warnings denied, all 69 tests (58 unit, 6 CLI and 5
documentation), debug/release builds and overview/detailed help checks passed.
The helpers are excluded from normal builds; networking remains unimplemented.

### 3. Frame headers

Encode/decode the 12-byte header using big-endian integers and the version 1
type registry. Reject unsupported versions, nonzero flags, unknown types and
payload lengths above 65,536 before payload allocation, using checked size
arithmetic. Keep context-dependent stream-state checks for later steps.

Test exact bytes, maximum lengths and malformed headers using TR-03; rejected
headers must not trigger a scan for another frame boundary.

Completed: the [header codec](../../src/modules/transport/codec/README.md)
encodes exact 12-byte version 1 headers and rejects invalid versions, flags,
types, payload bounds and statically invalid type/stream-ID combinations.
Immutable headers expose checked tagged/untagged frame sizes without allocating
payloads or performing I/O. Tests cover pinned PING/PONG headers, independent
DATA expectations, size boundaries, all type/version/flag values and malformed
header lengths. Payload formats, integrity, stream state and actual connection
closure remain later work; full TR-03 acceptance is not yet claimed.
Formatting, Clippy with warnings denied, all 80 tests (68 unit, 6 CLI and 6
documentation), build and overview/detailed help checks passed against the
unchanged reference revision.

### 4. Control and error payloads

Encode/decode PING/PONG tokens, GOAWAY fields and ERROR/CANCEL payloads without
implementing their connection behavior yet. Validate exact lengths, registered
codes, valid UTF-8 diagnostics of at most 1,024 bytes and absence of trailing
bytes; allow `NO_ERROR` only in GOAWAY.

Test the payload cases from TR-11, TR-12 and TR-13, including invalid codes and
lengths. Diagnostic strings must never determine machine state or expose local
secrets; context-specific error routing is completed in step 16.

Completed: [control payload codecs](../../src/modules/transport/codec/control/README.md)
encode/decode exact PING/PONG tokens, GOAWAY fields and ERROR/CANCEL diagnostics,
with the complete code registry, bounded UTF-8 reasons, strict lengths and
NO_ERROR restricted to GOAWAY. Decoders borrow text without allocation; structured
trace events expose function flow without reasons or tokens. Public contracts
have companion documents and tests cover header/payload composition and logging.
[Bug 0001](../../CR2SE%20bugs/0001-diagnostic-byte-order-mark.md) records the
pseudocode's omitted BOM check; implementation follows the normative text rule.
Formatting, Clippy with warnings denied, all 92 tests (77 unit, 8 integration
and 7 documentation), build, API documentation and overview/detailed help passed.
[Production line coverage](../../docs/coverage.md) is 93.64% (412/440), excluding
test-only code; new control codecs cover 166/166 lines. Connection behavior and
full TR-11/12/13 acceptance remain pending.

### 5. Complete frame I/O

Read and write complete header/payload/tag records over the byte-stream
interface, handling partial progress and several frames arriving in one read.
Accept an explicit per-direction integrity phase so the same framing code can
later handle the AUTH-to-tagged-frame transition. Serialize complete writes so
bytes from different frames cannot mix, and fail zero-progress writes.

Test every split point, one-byte reads, short writes, coalesced frames and
truncation within headers, payloads and tags using TR-02 and TR-14. Preserve
the distinction between EOF at a frame boundary and a truncated frame;
connection state later decides which active operations must fail.

Completed: [bounded frame I/O](../../src/modules/transport/framing/README.md)
handles partial/coalesced operations, explicit tag phases, terminal framing errors
and single-frame write ownership. All 99 tests, formatting, Clippy, build and
help checks passed; production line coverage is 94.83% (569/600). Returned
frames remain unverified; connection-state and integrity behavior are later steps.

### 6. HELLO and AUTH formats

Encode/decode the exact 100-byte HELLO and 64-byte AUTH payloads, including
version/algorithm selectors, public keys and nonce fields. Validate shape and
selectors here while delegating identity/signature semantics to later steps.

Test fixed byte layouts and incorrect lengths/selectors from TR-01 and TR-04;
parsing an identity key or signature must not mark the peer authenticated.

Completed: HELLO/AUTH codecs validate exact upstream layouts and selectors while leaving authentication to orchestration. All 102 tests, formatting, Clippy, build and help checks passed; production line coverage is 95.22% (617/648). Coverage exposed an existing parallel trace-test initialization race, fixed by serializing those two integration tests and verified with ten repeat runs.

### 7. Authentication inputs and key derivation

Construct the role-ordered HELLO transcript, signature messages, salt inputs,
HKDF labels, directional key/prefix assignments and nonce inputs exactly as
specified. Request primitive operations through the Cryptography interface and
identity signing through the Identity interface, without implementing either
module here.

Use recording substitutes and upstream fixtures to check exact input bytes,
roles, selectors, lengths and assignments for TR-01, TR-04 and TR-06. Compare
real signatures, derived keys and tags with the fixed vectors only when the
real Cryptography/Identity adapters are available in roadmap steps 3–5.

Completed: Canonical transcripts, role-bound signing/verification requests, exact HKDF labels and directional assignments are implemented with opaque secret ownership. Scripted tests verify exact inputs, failures and release behavior; real crypto remains pending. All 105 tests and required checks passed; production line coverage is 95.81% (709/740).

### 8. Handshake sequencing and identity

Drive HELLO/AUTH exchange for both TCP roles, requiring fresh ephemeral material
and nonce requests for each connection. Enforce the expected ID for known
contacts, accept same-identity peers, reject invalid order/duplicates/signatures
and all-zero shared secrets, and emit an authenticated event only after all
handshake checks succeed.

Track the tag transition separately in each direction and handle AUTH followed
by a protected frame in one read. Test TR-04 and TR-05, including the ban on
ordinary application frames before verifying remote AUTH; integrate actual tag
verification in step 9 and final timeout/error behavior in steps 16 and 19.

Completed: Handshake state enforces HELLO/AUTH ordering, pinned/same identities, fresh material requests and independent tag transitions. Authenticated facts are transferred only after verification and local AUTH completion. All 111 tests and required checks passed; production coverage is 96.53% (863/894). Bug 0002 records the upstream derivation-order contradiction; the normative post-verification order is followed.

### 9. Integrity and replay protection

Attach and verify each post-AUTH frame's 16-byte tag over the exact header and
payload through the Cryptography interface. Maintain independent 64-bit send
and receive sequences, starting at zero, with directional nonce prefixes and
no wrapping or reuse across connections. Integrity does not encrypt payloads.

With controlled verifier results, test TR-05 and TR-06: advance receive state
only after complete verification, dispatch nothing on failure, and close
without sending an error on tag failure. Exercise wrong directions, replay,
old-connection material and exhaustion; retain real mutation/vector tests as
explicitly pending until real cryptography is integrated.

Completed: Directional integrity state authenticates exact header/payload bytes, advances counters only after successful verification or complete writes, and releases keys on integrity failure or sequence exhaustion. Unit tests cover fixed-vector requests, replay rejection, wrong-direction tags, malformed tag presence and counter boundaries. Formatting, Clippy, all 116 tests, build and overview/detailed help passed; production line coverage is 96.91% (972/1003). Scripted primitives do not establish real cryptographic correctness.

### 10. Stream-opening payloads

Encode/decode OPEN with its protocol identifier and version. Validate the
1–255-byte identifier, lowercase ASCII grammar, dot rules, nonzero version and
absence of trailing bytes, without assigning new wire frame types.

Test valid and malformed identifiers/versions from TR-08. Keep malformed OPEN
separate from a valid but unsupported protocol so later routing emits the
correct error after consuming the stream identifier.

Completed: OPEN encoding and borrowed decoding enforce exact widths, ASCII namespace grammar and nonzero versions, preserving malformed versus unsupported selection. Independent wire expectations and malformed-input tests pass. All 119 tests, formatting, Clippy, build and help checks passed; production coverage is 97.07% (1027/1058).

### 11. Stream identifier allocation

Allocate initiator IDs as 1, 3, 5 and acceptor IDs as 2, 4, 6, checking exact
remote sequence and parity before accepting OPEN payloads. Consume rejected
OPEN IDs, forbid reuse/gaps and distinguish terminal IDs from never-created
IDs without retaining every closed stream forever.

Test TR-07, including concurrency around allocation and boundary values. Detect
exhaustion without wrapping; connect that result to `ID_EXHAUSTED` GOAWAY in
step 18. Handles must remain scoped to their connection instance.

Completed: Stream numbering now enforces role parity and exact next IDs, consumes rejected OPEN IDs through high-water marks, distinguishes previously created IDs without tombstones and detects exhaustion without wrapping. Tests cover both roles, serialized concurrent allocation, refusal history and integer boundaries; existing handle tests verify instance scope. All 123 tests and required checks passed; production coverage is 97.18% (1070/1101).

### 12. Protocol registration and dispatch

Register authorized consumers by protocol identifier/version and deliver new
streams with authenticated connection context. Reject a valid unregistered
protocol with `UNSUPPORTED_PROTOCOL` and malformed OPEN with `PROTOCOL_ERROR`,
while preserving other usable streams.

Test TR-08 with a protocol stub, including data arriving immediately after OPEN
and an ID consumed by refusal. Queue consumer events without invoking
re-entrant application code while internal locks are held; implementing Peer
Protocol itself remains a later module.

Completed: Authorized registrations route exact protocol versions to scoped consumers, queue authenticated stream-open events and consume malformed/unsupported OPEN IDs before stream refusal. All 126 tests and required checks passed; production coverage is 96.53% (1225/1269). DATA immediately following OPEN is exercised by the following data step; the full socket driver remains pending.

### 13. Bidirectional stream data

Allow both peers to send bounded DATA on open directions and interleave frames
from different streams while preserving each direction's byte order. Expose
the concatenated byte stream to consumers, without treating DATA frame
boundaries as application message boundaries.

Test TR-03 and TR-09 with both peers opening streams and exchanging differently
sized data, including the maximum frame payload and invalid empty DATA. Keep
buffers bounded from the outset; configurable limits are completed in step 20.

Completed: DATA supports both stream creators, immediate bytes after OPEN, independent interleaving and ordered byte concatenation. Full-size frames, authority checks, invalid empty data and bounded queue overflow are tested. All 129 tests and required checks passed; production coverage is 96.65% (1297/1342). Configurable instance-wide limits remain step 20.

### 14. Half-close and normal completion

Handle local and remote END independently, leaving the opposite direction
usable until it also ends. Report normal completion only after END in both
directions, and reject DATA or repeated END after the sender's own END.

Test both half-close orders, nonempty END payloads and isolated stream-state
errors from TR-09. Use TR-14 to ensure EOF cannot turn an unfinished stream
into a normal completion, even when expected-looking bytes have arrived.

Completed: Independent local/remote END state preserves the opposite direction and emits completion only after remote END plus a fully written local END. Tests cover both close orders, repeated/nonempty END, DATA after END and isolated stream errors. All 131 tests and required checks passed; production coverage is 96.80% (1360/1405). EOF cleanup is composed in the following error/closure steps.

### 15. Cancellation and terminal streams

Make CANCEL and stream ERROR terminate both stream directions, notify the
consumer and stop local production. Discard later non-OPEN frames for terminal
streams while continuing to enforce connection framing and integrity checks;
an OPEN reusing an old ID remains invalid.

Test queued DATA/END after cancellation, repeated terminal controls and reuse
from TR-10. Never translate cancellation into an application retry, credit
rollback or a claim that remote work did not happen.

Completed: Local and peer cancellation/error terminate both directions once, stop queued production and preserve unsent OPEN ordering. Verified terminal non-OPEN frames are discarded; never-created references and reused OPENs remain connection violations. All 134 tests and required checks passed; production coverage is 96.16% (1427/1484).

### 16. Error scope and responses

Connect parser/state failures to the specified wire codes and local outcomes,
distinguishing an isolated active-stream failure from a connection failure.
Validate type/stream combinations, unknown stream references and malformed
control payloads, and fail every affected consumer on a connection error.

Test the error matrices from TR-03, TR-04, TR-08, TR-09, TR-13 and TR-14.
Send best-effort connection ERROR only when encoding it is safe; unsupported
versions, failed tags and uncertain integrity/framing state require closure
without that response. Preserve completed stream results when a later error
closes the connection.

Completed: Failure dispositions preserve typed causes and safe wire codes, suppress unsupported-version/tag/uncertain-output replies, and handle peer ERROR without reply loops. Session closure fails unfinished consumers once while retaining completed outcomes. All 139 tests and required checks passed; production coverage is 95.42% (1541/1615). Socket-level application of this policy is composed in deadline/cleanup step 19.

### 17. Ping and response matching

Answer authenticated PING with exactly one matching PONG and track pending
local ping tokens with configurable deadlines. Ignore unsolicited or duplicate
PONG without treating unrelated tokens as success.

Test TR-11 using the controllable clock, including mismatched tokens,
duplicates and expiration without real sleeps. A successful ping is evidence
of reachability, not a service acknowledgement.

Completed: Timed probes use injected monotonic time, match only pending tokens, echo PING once and ignore unsolicited/duplicate/expired PONG. Pending probes are bounded and failed on closure. All 141 tests and required checks passed; production coverage is 95.53% (1625/1701).

### 18. Graceful shutdown

Implement sending/receiving GOAWAY, validate the accepted-stream boundary and
stop new local streams while accepted work drains. Refuse otherwise valid new
remote OPEN requests during draining, reject a repeated GOAWAY that raises
its boundary, and wire stream-ID exhaustion to `ID_EXHAUSTED` shutdown.

Test TR-07 and TR-12, including boundary parity/range, refused higher IDs and
closure only after accepted streams finish or a separate failure requires it.
Do not automatically replay refused application operations.

Completed: GOAWAY enforces local parity, greatest-opened and non-increasing acceptance bounds, refuses higher local streams and consumes/refuses late remote OPENs. Local exhaustion starts ID_EXHAUSTED drain; closure readiness waits for accepted streams and complete GOAWAY output. All 144 tests and required checks passed; production coverage is 95.62% (1701/1779).

### 19. Deadlines and connection cleanup

Enforce handshake deadlines with the monotonic clock and make all closure
paths release sockets, queued work, ephemeral credentials and session-secret
handles. Fail unfinished streams once, retain completed outcomes and prevent
old handles or session state from being used on another connection.

Test TR-06, TR-14 and TR-15 across timeout, EOF, I/O failure, failed handshake
and normal drain. Verify cleanup through test adapters now; actual secret
erasure and fresh-session behavior across restart must also be checked when
the real adapters are integrated.

Completed: The owned connection driver now composes framing, handshake, integrity and stream dispatch with bounded polling, handshake/error-output deadlines, stable instance authority and deterministic socket/key cleanup. Tests cover both roles, coalesced handshake bytes, stalled deadlines, EOF/truncation, failed writes, safe versus silent errors and unfinished stream failure. All 148 tests and required checks passed; production coverage is 93.88% (1965/2093).

### 20. Resource limits

Expose finite local limits for concurrent connections, active streams, total
outbound bytes and per-stream buffered bytes. Check limits before admitting
work or allocating buffers, preserve support for the full protocol frame size,
and use backpressure or the specified resource failure when capacity is full.

Test TR-15 at and beyond each boundary, including cancellation/drain while
queues are full and recovery of released capacity. Resource refusal must not
look like normal completion; connection-level limits must also cover the TCP
admission path added in step 22.

Completed: Shared reservations enforce connection admission and serialized queued/in-flight byte limits across connection owners. Sessions enforce configured active-stream and consumer-byte limits plus finite metadata events; cancellation releases queued capacity and resource refusal remains distinct from completion. All 152 tests and required checks passed; production coverage is 93.32% (2097/2247). TCP admission uses these same shared budgets in step 22.

### 21. Fair scheduling under backpressure

Schedule complete frames across ready streams so a large transfer or stalled
consumer cannot permanently starve smaller operations. Maintain finite queues,
per-stream send ordering and one serialized frame writer while applying local
backpressure or cancelling an affected stream when safe.

Test TR-02, TR-09 and TR-15 with short writes, a blocked consumer and several
active streams; assert progress where the socket permits it and bounded memory
where it does not. Do not invent wire-level flow-control frames.

Completed: Whole-frame round-robin scheduling preserves per-stream order, global OPEN numbering and GOAWAY barriers. Independent consumer polling and per-stream overflow isolation prevent a blocked consumer from stopping unrelated progress; the existing partial writer keeps frame bytes contiguous and budgeted. All 155 tests and required checks passed; production coverage is 93.40% (2138/2289).

### 22. TCP adapters

Implement outgoing connect and incoming listen/accept around the tested
byte-stream interface, supplying explicit identities, expected contact IDs,
limits and deadlines. Handle connection refusal, bind errors, shutdown and
socket failures through the established Transport outcomes.

Use OS-assigned loopback ports for adapter integration tests and keep scripted
authentication inside the test harness. Leave the normal CLI's unavailable
server behavior in place until real Identity/Cryptography can be supplied;
the authenticated two-node run belongs to roadmap step 5.

Completed: Mio-backed nonblocking TCP adapters and monotonic wakeups support outgoing attempts, listeners, accepted sockets and immediate cancellation/closure. The public control driver composes all requests with shared admission, scoped authority, fair connection polling and earliest-deadline registration. Loopback tests use ephemeral ports and cover real I/O, bind/refusal failures and timers; scripted driver tests cover incoming ordering and capacity recovery. All 161 tests and required checks passed; production coverage is 92.42% (2757/2983). Normal CLI serve remains unavailable until real credentials/cryptography are supplied.

### 23. Acceptance verification

Map TR-01 through TR-15 to executable tests and run the composed handshake,
framing, stream, control and failure paths using the deterministic harness.
For malformed protected fixtures, make the integrity check succeed unless the
test specifically targets integrity, so the intended rule is actually reached.

Record which assertions pass with substitutes and which require real crypto,
real identity/session lifecycle or an independent verifier. Do not mark those
pending assertions passed because a scripted adapter returned a fixture value.
Run `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked` and `cargo build --locked`, then repeat the affected
acceptance/vector cases with real dependencies in roadmap steps 3–5.

### 24. Documentation and integration handoff

Update the Transport README, root README, CLI help and both plans with the
implemented boundary, build/test/run commands, remaining limits and acceptance
results. Explain what can be exercised independently and what still needs real
Identity/Cryptography; do not claim a working authenticated server prematurely.

Check `cargo run --locked -- --help` and `cargo run --locked -- help serve`,
record the upstream revision used and link any confirmed protocol bug reports.
Only check completed substeps, preserving the later integration milestone and
all pending real-cryptography checks in the main roadmap.

[spec]: https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md
[pseudocode]: https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/pseudocode/README.md
[inputs]: https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/inputs/README.md
[outputs]: https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/outputs/README.md
[contracts]: https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/ModuleContracts.md
[acceptance]: https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/acceptanceTests/README.md
[vectors]: https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/pseudocode/TestVectors.md
