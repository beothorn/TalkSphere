# Transport module plan

Status: proposed for review; no implementation steps have been started.

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

- [ ] **1.** Define Transport's public interface and replaceable dependencies.
- [ ] **2.** Add deterministic test helpers for Transport's dependencies.
- [ ] **3.** Implement frame header encoding and validation.
- [ ] **4.** Implement connection-control and error payload formats.
- [ ] **5.** Read and write complete frames across partial socket operations.
- [ ] **6.** Implement HELLO and AUTH handshake payload formats.
- [ ] **7.** Construct authentication transcripts and directional key derivation requests.
- [ ] **8.** Implement handshake sequencing and peer identity checks.
- [ ] **9.** Add frame integrity verification and replay protection.
- [ ] **10.** Implement stream-opening payload encoding and validation.
- [ ] **11.** Allocate and validate stream identifiers in the required order.
- [ ] **12.** Route new streams to registered protocol handlers.
- [ ] **13.** Exchange ordered data across multiple bidirectional streams.
- [ ] **14.** Support independent stream half-closing and normal completion.
- [ ] **15.** Implement stream cancellation and terminal-state handling.
- [ ] **16.** Apply the required stream-level and connection-level error responses.
- [ ] **17.** Add ping requests with response matching and timeouts.
- [ ] **18.** Implement graceful connection shutdown with GOAWAY.
- [ ] **19.** Enforce handshake deadlines and release connection resources on closure.
- [ ] **20.** Enforce limits on connections, streams and queued bytes.
- [ ] **21.** Schedule active streams fairly when consumers or sockets are slow.
- [ ] **22.** Add TCP adapters for outgoing and incoming connections.
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

### 2. Deterministic test helpers

Provide test-owned sockets that split reads/writes, combine incoming frames,
fail writes and end at chosen byte offsets, plus a manually advanced clock and
a recording protocol consumer. Add scripted Identity/Cryptography substitutes
that record exact inputs and can reject signatures, shared secrets or tags.

Keep substitutes available only to tests; do not expose a runtime switch for
fake authentication. Test the helper behaviors used by assertions and retain
upstream fixtures with their revision and expected bytes. Scripted results can
verify orchestration but cannot prove cryptographic correctness.

### 3. Frame headers

Encode/decode the 12-byte header using big-endian integers and the version 1
type registry. Reject unsupported versions, nonzero flags, unknown types and
payload lengths above 65,536 before payload allocation, using checked size
arithmetic. Keep context-dependent stream-state checks for later steps.

Test exact bytes, maximum lengths and malformed headers using TR-03; rejected
headers must not trigger a scan for another frame boundary.

### 4. Control and error payloads

Encode/decode PING/PONG tokens, GOAWAY fields and ERROR/CANCEL payloads without
implementing their connection behavior yet. Validate exact lengths, registered
codes, valid UTF-8 diagnostics of at most 1,024 bytes and absence of trailing
bytes; allow `NO_ERROR` only in GOAWAY.

Test the payload cases from TR-11, TR-12 and TR-13, including invalid codes and
lengths. Diagnostic strings must never determine machine state or expose local
secrets; context-specific error routing is completed in step 16.

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

### 6. HELLO and AUTH formats

Encode/decode the exact 100-byte HELLO and 64-byte AUTH payloads, including
version/algorithm selectors, public keys and nonce fields. Validate shape and
selectors here while delegating identity/signature semantics to later steps.

Test fixed byte layouts and incorrect lengths/selectors from TR-01 and TR-04;
parsing an identity key or signature must not mark the peer authenticated.

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

### 10. Stream-opening payloads

Encode/decode OPEN with its protocol identifier and version. Validate the
1–255-byte identifier, lowercase ASCII grammar, dot rules, nonzero version and
absence of trailing bytes, without assigning new wire frame types.

Test valid and malformed identifiers/versions from TR-08. Keep malformed OPEN
separate from a valid but unsupported protocol so later routing emits the
correct error after consuming the stream identifier.

### 11. Stream identifier allocation

Allocate initiator IDs as 1, 3, 5 and acceptor IDs as 2, 4, 6, checking exact
remote sequence and parity before accepting OPEN payloads. Consume rejected
OPEN IDs, forbid reuse/gaps and distinguish terminal IDs from never-created
IDs without retaining every closed stream forever.

Test TR-07, including concurrency around allocation and boundary values. Detect
exhaustion without wrapping; connect that result to `ID_EXHAUSTED` GOAWAY in
step 18. Handles must remain scoped to their connection instance.

### 12. Protocol registration and dispatch

Register authorized consumers by protocol identifier/version and deliver new
streams with authenticated connection context. Reject a valid unregistered
protocol with `UNSUPPORTED_PROTOCOL` and malformed OPEN with `PROTOCOL_ERROR`,
while preserving other usable streams.

Test TR-08 with a protocol stub, including data arriving immediately after OPEN
and an ID consumed by refusal. Queue consumer events without invoking
re-entrant application code while internal locks are held; implementing Peer
Protocol itself remains a later module.

### 13. Bidirectional stream data

Allow both peers to send bounded DATA on open directions and interleave frames
from different streams while preserving each direction's byte order. Expose
the concatenated byte stream to consumers, without treating DATA frame
boundaries as application message boundaries.

Test TR-03 and TR-09 with both peers opening streams and exchanging differently
sized data, including the maximum frame payload and invalid empty DATA. Keep
buffers bounded from the outset; configurable limits are completed in step 20.

### 14. Half-close and normal completion

Handle local and remote END independently, leaving the opposite direction
usable until it also ends. Report normal completion only after END in both
directions, and reject DATA or repeated END after the sender's own END.

Test both half-close orders, nonempty END payloads and isolated stream-state
errors from TR-09. Use TR-14 to ensure EOF cannot turn an unfinished stream
into a normal completion, even when expected-looking bytes have arrived.

### 15. Cancellation and terminal streams

Make CANCEL and stream ERROR terminate both stream directions, notify the
consumer and stop local production. Discard later non-OPEN frames for terminal
streams while continuing to enforce connection framing and integrity checks;
an OPEN reusing an old ID remains invalid.

Test queued DATA/END after cancellation, repeated terminal controls and reuse
from TR-10. Never translate cancellation into an application retry, credit
rollback or a claim that remote work did not happen.

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

### 17. Ping and response matching

Answer authenticated PING with exactly one matching PONG and track pending
local ping tokens with configurable deadlines. Ignore unsolicited or duplicate
PONG without treating unrelated tokens as success.

Test TR-11 using the controllable clock, including mismatched tokens,
duplicates and expiration without real sleeps. A successful ping is evidence
of reachability, not a service acknowledgement.

### 18. Graceful shutdown

Implement sending/receiving GOAWAY, validate the accepted-stream boundary and
stop new local streams while accepted work drains. Refuse otherwise valid new
remote OPEN requests during draining, reject a repeated GOAWAY that raises
its boundary, and wire stream-ID exhaustion to `ID_EXHAUSTED` shutdown.

Test TR-07 and TR-12, including boundary parity/range, refused higher IDs and
closure only after accepted streams finish or a separate failure requires it.
Do not automatically replay refused application operations.

### 19. Deadlines and connection cleanup

Enforce handshake deadlines with the monotonic clock and make all closure
paths release sockets, queued work, ephemeral credentials and session-secret
handles. Fail unfinished streams once, retain completed outcomes and prevent
old handles or session state from being used on another connection.

Test TR-06, TR-14 and TR-15 across timeout, EOF, I/O failure, failed handshake
and normal drain. Verify cleanup through test adapters now; actual secret
erasure and fresh-session behavior across restart must also be checked when
the real adapters are integrated.

### 20. Resource limits

Expose finite local limits for concurrent connections, active streams, total
outbound bytes and per-stream buffered bytes. Check limits before admitting
work or allocating buffers, preserve support for the full protocol frame size,
and use backpressure or the specified resource failure when capacity is full.

Test TR-15 at and beyond each boundary, including cancellation/drain while
queues are full and recovery of released capacity. Resource refusal must not
look like normal completion; connection-level limits must also cover the TCP
admission path added in step 22.

### 21. Fair scheduling under backpressure

Schedule complete frames across ready streams so a large transfer or stalled
consumer cannot permanently starve smaller operations. Maintain finite queues,
per-stream send ordering and one serialized frame writer while applying local
backpressure or cancelling an affected stream when safe.

Test TR-02, TR-09 and TR-15 with short writes, a blocked consumer and several
active streams; assert progress where the socket permits it and bounded memory
where it does not. Do not invent wire-level flow-control frames.

### 22. TCP adapters

Implement outgoing connect and incoming listen/accept around the tested
byte-stream interface, supplying explicit identities, expected contact IDs,
limits and deadlines. Handle connection refusal, bind errors, shutdown and
socket failures through the established Transport outcomes.

Use OS-assigned loopback ports for adapter integration tests and keep scripted
authentication inside the test harness. Leave the normal CLI's unavailable
server behavior in place until real Identity/Cryptography can be supplied;
the authenticated two-node run belongs to roadmap step 5.

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
