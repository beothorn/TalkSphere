# Transport acceptance coverage

Baseline: [CR2SE 8e848eb2e70ef7cd4490a753f9e647fdddcd99fd](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd),
[TR-01–TR-15](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/acceptanceTests/README.md).

The planned Transport boundary is exercised with deterministic dependency
substitutes, plus real loopback TCP adapter tests. These results establish local
codec/state/orchestration behavior, not cryptographic conformance or an
authenticated deployment. Real Cryptography/Identity integration remains
[roadmap steps 3–5](../../../plans/PLAN.md).

## Executable mapping

Run all cases from the repository root with `cargo test --locked`.
The composed suite is
`cargo test --locked modules::transport::connection::tests::acceptance`.
Its [source](connection/tests/acceptance.rs) drives actual socket polling,
framing, handshake, integrity requests, stream dispatch and closure. Malformed
protected fixtures explicitly pass their scripted verifier unless integrity
rejection is the property being tested.

| Case | Local executable coverage | Remaining independent evidence |
| --- | --- | --- |
| TR-01 | `handshake/tests.rs`, `handshake/state/tests.rs`, `integrity/tests.rs`, composed byte-segmented handshake: exact layouts, transcript/signing/KDF requests, directional labels and authenticated IDs | Compute signatures, keys, nonce prefixes and tags with real primitives against pinned vectors |
| TR-02 | `framing/tests.rs` checks every fixture split and short write; composed `every_protected_split_and_coalesced_frames` and `byte_segmented_handshake_and_protected_tail_with_short_writes` check ordered dispatch and exact output | Repeat with real authenticated frames |
| TR-03 | Header/DATA codec boundaries and composed `malformed_header_response_matrix`; full 65,536-byte receive/send and no dispatch on rejected headers | Real tag calculation for maximum frames |
| TR-04 | Handshake codec/state tests cover order, widths, selectors, pinning and scripted signature/agreement rejection; connection tests verify safe versus silent closure | Actual invalid Ed25519 proofs and all-zero X25519 rejection in Crypto adapter |
| TR-05 | Role-specific handshake gates, coalesced AUTH/tagged input and same-identity state tests; composed byte-segmented transition | Fresh real ephemeral material for same-identity peers |
| TR-06 | Integrity tests verify exact AAD/nonces, independent counters, scripted replay/wrong-direction rejection and exhaustion; composed `failed_integrity_closes_silently` verifies no output/dispatch and key release | Real header/payload/tag mutations, replay, reversed keys/prefixes, old-session tags and secret erasure |
| TR-07 | `stream/ids/tests.rs` covers parity, exact sequence, concurrency and exhaustion; stream/session and composed state violations cover consumed/refused IDs, never-created references and GOAWAY exhaustion | Repeat on real two-node sessions |
| TR-08 | OPEN codec malformed-input matrix, registry/session routing and composed `routing_halves_cancellation_and_reuse_through_socket` | Repeat with real protected input |
| TR-09 | Session interleaving/half-close matrix plus composed full-size DATA and opposite-direction reply after END | Real two-node bidirectional exchange |
| TR-10 | Session cancellation/terminal tests and composed cancellation/reuse scenario; no retry/ledger calls exist | Integration must retain this behavior with later application modules |
| TR-11 | Session token/deadline tests and composed `ping_matching_timeout_and_graceful_empty_drain`; exact echo, duplicates/unrelated responses and timeout | Repeat across real nodes |
| TR-12 | Session bound/parity/repetition tests and composed `goaway_refusal_and_accepted_stream_completion`; late OPEN refusal and normal drain | Real socket drain between authenticated nodes |
| TR-13 | Control codec diagnostic matrix, failure dispositions, peer ERROR and composed malformed protected controls | Real tags over independently malformed control vectors |
| TR-14 | Every partial framing prefix and composed `every_incomplete_protected_prefix_is_truncation`; EOF fails unfinished streams and preserves completed outcomes | Repeat across real session lifecycle/restart |
| TR-15 | Manual-clock handshake expiry, shared budgets across partial writes, connection/listener admission, fair scheduler and composed slow-consumer isolation | Real secret erasure/freshness and workload sizing for deployment |

## Validation record

Step 23: 172 tests passed (157 unit, 8 integration and 7 documentation tests).
Formatting, Clippy with warnings denied, locked build and overview/serve help
passed. Production line coverage was **94.00% (2804/2983)** using the
[production-only procedure](../../../docs/coverage.md). Test-only code and
fixtures are excluded. Coverage is not a substitute for independent wire or
cryptographic verification.

The fixed fixture snapshot records its upstream revision under
[test_support/fixtures](test_support/fixtures/README.md). Strict scripts assert
exact primitive requests; they never silently accept unspecified cryptographic
operations. TCP tests use OS-selected loopback ports and real readiness wakeups,
without exposing scripted authentication through the application.

## Protocol findings and handoff

- [0001: diagnostic BOM omission](../../../CR2SE%20bugs/0001-diagnostic-byte-order-mark.md): codec follows the normative prohibition.
- [0002: key derivation order](../../../CR2SE%20bugs/0002-key-derivation-before-auth-verification.md): handshake follows normative post-verification derivation.

Keep these findings distinct from local Rust choices. Before claiming full
conformance, integrate real primitives and credentials, compare every pinned
cryptographic output, run mutation/replay/restart cases and demonstrate two
local nodes in both directions. No test substitute is an authenticated service.
