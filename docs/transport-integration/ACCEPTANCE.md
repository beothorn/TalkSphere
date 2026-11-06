# Server integration acceptance evidence

Implementation baseline: [CR2SE d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
The upstream comparison against main on 2026-11-05, rechecked on 2026-11-06, is six commits ahead and changes
only website/workflow/root introductory material; no module contracts, pseudocode,
acceptance cases or vectors changed. Retain the existing behavior and fixture revision.
The specification, Transport inputs/outputs, pseudocode, vectors, TR suite,
ModuleContracts and shared acceptance guide were re-read at that revision.

The earlier [Transport matrix](../../src/modules/transport/ACCEPTANCE.md) has historical
pending notes superseded by its Cryptography and persistent Identity supplements.
Real AUTH and directional frame vectors already pass in composition tests, using
protected identities loaded from disk. They do not yet exercise a server process.

| Case | Executed assertion evidence | Result / binding |
| --- | --- | --- |
| TR-01 | real_wire::segmented_handshake_and_tag_transition_match_vectors; independent_tag_oracle_matches_pinned_first_frame; composition::identity pinned handshake/frame test; server_peers exact authenticated IDs/roles | Pass: literal vectors/native primitives, controlled bytes, actual TCP/processes |
| TR-02 | every_protected_split_preserves_real_authenticated_bytes; segmented_handshake_and_tag_transition_match_vectors; existing framing every-split tests | Pass: every OPEN/DATA split, one-byte handshake/tail and 1–3-byte writes; real TCP payload tests supplement controlled segmentation |
| TR-03 | native_stream_allocation_and_protocol_selection_are_exact; routing_halves_cancellation_and_reuse_through_socket; malformed_header_response_matrix; real_tcp_resource_admission_and_frame_fairness | Pass: lengths 1/65536, oversized/invalid headers before dispatch; actual TCP maximum/small frames |
| TR-04 | real_handshake_failure_precedes_agreement_for_both_roles; malformed_and_timed_out_handshakes_do_not_publish_keys; partial_auth_write_and_expected_identity_failure_do_not_derive; handshake/state ordering_duplicates_and_payload_failures_are_terminal | Pass: native two-role negatives/order observations, complete state/width/duplicate matrix at the state boundary |
| TR-05 | segmented_handshake_and_tag_transition_match_vectors; partial_auth_write_and_expected_identity_failure_do_not_derive; server_runtime freshness and server_peers same-identity tests | Pass: tag transition, withheld proof, partial AUTH, same-ID roles and independently fresh public handshake material |
| TR-06 | integrity_mutation_replay_and_direction_confusion_close_silently; old_connection_tag_is_rejected_under_new_signed_transcript; native_connection_failure_releases_acquired_secret_owners; integrity sequence_exhaustion_closes_without_reusing_a_nonce | Pass: native rejection/release; extreme sequence boundary uses controlled unit state |
| TR-07 | native_stream_allocation_and_protocol_selection_are_exact; native_stream_id_violations_close_connection; real_tcp_multiplexing_halves_and_cancellation; ids both_parities_exhaust_without_wrapping and session exhausted_numbering_drains | Pass: real allocation in both parities, real-tag failures; uint32 exhaustion uses controlled unit state |
| TR-08 | native_stream_allocation_and_protocol_selection_are_exact; control/open codec matrices; routing consumers_are_isolated_and_failure_requests_cancellation | Pass: exact selection/refusal/consumed IDs and ownership; isolated test registration only |
| TR-09 | real_tcp_multiplexing_halves_and_cancellation; native routing_halves_cancellation_and_reuse_through_socket; session independent_half_close_orders_and_write_completion and invalid_peer_half_close_sequences_are_isolated | Pass: actual bidirectional TCP plus native/state transition matrix |
| TR-10 | real_tcp_multiplexing_halves_and_cancellation; native terminal/cancellation scenario; session cancellation_stops_production_and_discards_terminal_frames | Pass: cancellation, queued terminal discard, reuse rejection and independent progress; no Ledger/retry dependency |
| TR-11 | real_tcp_probes_and_accepted_stream_drain; native ping_matching_timeout_and_graceful_empty_drain; session ping_echo_matching_and_timeout | Pass: both TCP probe directions, exact token/unsolicited/duplicate/unrelated/timeout state assertions |
| TR-12 | real_tcp_probes_and_accepted_stream_drain; native goaway_refusal_and_accepted_stream_completion, protected_state_violations_close_connection, eof_after_completed_goaway_is_normal and peer_drain_reason_is_preserved; session goaway_bounds_and_late_open_refusal | Pass: actual accepted-stream drain and complete bound/refusal matrix |
| TR-13 | native_diagnostic_reason_vectors_reach_control_validation; malformed_protected_controls_are_connection_errors; shared control codec tests | Pass: correctly tagged malformed controls, all literal reason rows, exact accepted bytes and byte-limit/BOM checks |
| TR-14 | native every_incomplete_protected_prefix_is_truncation and EOF-after-drain; session connection_failure_preserves_completed_outcomes; framing every-prefix tests | Pass: partial header/payload/tag, no provisional dispatch/resynchronization, failed unfinished versus completed work |
| TR-15 | bounded_slow_consumer_does_not_stop_another_stream; real_tcp_resource_admission_and_frame_fairness; server_runtime silent-peer test; driver shared budgets/deadlines and session scheduler tests | Pass: bounded occupancy/refusal, timeout, whole-frame writes and independent progress; no performance SLA claimed |
| AT-C01 | Scoped credential/router tests, real Identity/Cryptography adapters and actual runtime tests | Pass for supported direct local binding; serialized local module deployment unsupported |
| AT-C02 | Config/CLI boundaries, native failure matrices, startup failures, readiness worker_failure_releases_waiters_and_preserves_error; Identity durability/reconciliation suite | Pass at actual supported boundaries; no automatic service execution/retry or fabricated remote acknowledgement |
| AT-C03 | Credential lock/snapshot tests, server_peers restart, server_runtime refresh and ownership tests, protected Identity restart suite | Pass: stable durable identity, fresh sessions and immutable copied inputs; transient connection handles are not restart authority |
| AT-C04 | server_trace, native_runtime_traces_exclude_payloads_keys_and_peer_diagnostics, pre-start cancellation, CLI help/exit tests | Pass: complete validation precedes status, no private/payload diagnostics, no hidden optional module calls |

The matrix combines the unit/state boundary cases with native connection, actual
TCP and separate-process evidence; it does not pretend every extreme/malformed case
was sent by an executable peer. Pure codec and controlled-counter cases remain
appropriate for exactness and impractical limits. The full locked test command runs
all these assertions, including earlier module suites. Supporting detail follows.

The application owns runtime lifecycle under src/composition; Transport owns wire
state; Identity owns protected credentials; Cryptography owns primitive execution.
No automatic identity replacement, generic signing interface, fake cr2se.peer
handler, service retry or later module behavior is authorized by this milestone.
Independent implementation interoperability, paid recovery and minimum-useful-node
conformance remain later work. No new protocol defect was confirmed by this review.

## Separate-process authentication

`cargo test --locked --test server_peers` creates two distinct protected identities,
starts the actual executable twice and verifies both reported authenticated IDs and
TCP roles. Both processes are restarted with their roles reversed and retain their
public identities. Readiness and shutdown are observed with bounded pipe reads;
SIGTERM completes normally. This is same-implementation process evidence, not
independent interoperability or a service protocol demonstration.

## Real protected framing

The server real_wire suite uses persisted/reloaded native Identity and real
Cryptography through Connection's actual framing/session boundary. Only socket
segmentation and monotonic time are controlled. An independent direct AEAD oracle
matches the literal upstream tag, then supplies correct tags for state tests.
Every OPEN/DATA split and one-byte handshake/coalesced protected tail run with
one-to-three-byte writes and exact pinned HELLO/AUTH output assertions. Real socket
and separate-process behavior are separately covered by server_peers and runtime tests.

## Authentication failure ordering

The real_wire crypto observer delegates actual native results and asserts at each
agreement call that successful peer verification and all 188 local HELLO/AUTH bytes
preceded it. Both roles reject bad signatures, zero-ephemeral plus invalid AUTH,
small-order public/R points and noncanonical S before agreement/HKDF. A valid
signature over the actual zero-ephemeral transcript reaches agreement, rejects zero
and never extracts keys. Ordering/selector/width failures and withheld AUTH timeout
publish no session or authenticated event. Existing state tests retain the complete
duplicate/width matrix; the real persisted adapter also checks contact-ID mismatch.

## Native integrity rejection

real_wire verifies header/payload/tag mutation, replay at the next sequence,
reversed direction key and nonce prefix, and an old connection tag after a newly
signed transcript changes the session keys. Each closes without output or payload
routing. Release observations count all eleven acquired native credential/session
owners after integrity failure and drop; compiler/register copies are outside this
observation. Existing controlled-state sequence exhaustion tests retain wrap/no-nonce-
reuse assertions without impractical traffic or production counter setters.

## Native stream allocation and protocol selection

real_wire checks local IDs 1/3/5 and remote IDs 2/4/6, correct real tags over
malformed/unknown protocol OPENs, consumption of refused IDs, and continued use of
the next stream. Wrong parity, skips, reuse and never-created DATA close the
connection after native integrity checks. The original allocator's controlled uint32
exhaustion tests retain GOAWAY/no-wrap evidence; no production counter override is
introduced. Registrations here are isolated test consumers, not a production Peer
Protocol implementation or advertised application service.

## Real TCP streams and native terminal handling

server_streams runs two actual TCP drivers with independent protected keys and
bounded test-only consumers. It checks 1/3/5 allocation, interleaved exact bytes,
reply after half-close, normal completion, cancellation with another stream still
usable, and reverse-direction stream creation/half-close. The production executable
still registers no application handler. real_wire additionally repeats full-size
DATA, refused OPEN consumption, queued traffic after cancellation and terminal-ID
reuse rejection with independent native-tag response oracles.

## Probes and graceful drain

Native-tag fixtures repeat matching/unsolicited PONG and timeout behavior, GOAWAY
bound reductions, late OPEN refusal, invalid bounds, accepted-stream completion,
EOF after drain and typed peer drain reasons. server_streams verifies both probe
directions over actual TCP, rejects new OPEN after runtime shutdown begins, receives
a final reply on an accepted stream, preserves Completed outcomes and releases both
connection reservations. Process SIGTERM coverage separately verifies normal exit.

## Truncation and finite operation

Native-tag scenarios repeat invalid header bounds, malformed controls, every
incomplete protected DATA prefix and slow-consumer overflow with independent
stream progress. Literal diagnostic reason rows run for ERROR/CANCEL/GOAWAY with
valid tags, accepted byte preservation and leading-BOM rejection. A real silent TCP
peer is closed by its configured handshake deadline while the listener remains
available. Actual TCP driver tests refuse excess connection/stream admission,
inspect occupancy and deliver maximum-size plus small-stream traffic without
starvation. Existing shared-budget/short-write scheduler tests complement these
observations; none claims a throughput or latency service guarantee.

## Restart, operational failures and privacy

server_runtime reads actual public HELLO bytes across reconnect/restart: the signing
key stays fixed while both the ephemeral key and nonce change. server_peers also
accepts separately protected copies of the same identity in distinct TCP roles.
Configured missing/corrupt/bind-conflict failures preserve private records and
release ownership. A pre-start injected shutdown reports only Stopped after releasing
the store lock, without binding a listener.

The readiness worker's actual fatal-state transition is directly unit-tested: all
read/write/timer waiters wake, and later arming reports the stored error. This is
controlled state testing of OS-error handling, not a claim that the test destroys
an OS poll descriptor. Existing injected entropy tests retain dependency failure
and acquisition cleanup evidence; production CSPRNG failure is never simulated by
an insecure CLI switch or fallback.

Native TRACE capture excludes verified application bytes, peer diagnostic text and
fixture private material. server_trace drains real executable TRACE stderr in a
bounded collector, checks both stored private inputs in hex/array representations,
and verifies resource release before Stopped. No minimum-profile service recovery,
application replay or physical erasure is inferred from these observations.

## Completion verification

Integration steps 1–24 are complete for the supported Linux/direct binding.
Formatting, warnings-denied Clippy, locked unit/integration/doctests, build, API
documentation and all command/option/logging help topics pass. Production-only line
coverage is **96.38% (5359/5560)** using docs/coverage.md exclusions. All source
changes are retained as focused per-step commits with intent and verification.

The final upstream recheck still finds only website/workflow introductory changes
beyond d40b6278; protocol contracts and vectors are unchanged. No new confirmed
protocol defect required an issue. The local outgoing-admission/drain tracking bug
was fixed and tested in step 21; it is not recorded as a CR2SE issue.

Completion means authenticated Transport connections, bounded streams and runtime
lifecycle work with real modules and sockets. It does not mean application services,
serialized local-module deployment, independent interoperability, paid recovery,
minimum useful node or full-feature conformance. The next roadmap work is Board.
