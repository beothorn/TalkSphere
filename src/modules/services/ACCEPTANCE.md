# Services acceptance evidence

Reviewed on 2026-11-08 against
[CR2SE d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f), including
[SV-01–SV-12](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/acceptanceTests/README.md)
and [common contracts](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md).
Final comparison with upstream main found six website/workflow/introductory commits
and no changed protocol contracts. This is evidence for the local Services module;
it does not establish live peer interoperability or complete standard families.

From the repository root run `cargo test --locked modules::services`,
`cargo test --locked --example services`, and the checks in AGENTS.md. Test names
below are executable filters. Sources are in [tests](tests/) unless otherwise linked.

| Case | Assertions and executable evidence |
|---|---|
| SV-01 | `fabricated_support_marker_is_not_validation` rejects each missing public definition field despite a fabricated Known flag; Board `definitions_match_exact_fields` rejects missing check/schema descriptions, mismatched identities/version and economic fields; schema tests reject empty nested descriptions and preserve unknown types as unsupported; `catalogue_and_capacity` rejects reserved names without an independent catalogue; `real_board_selection_and_staleness` checks complete compatible contracts, not names alone. |
| SV-02 | `literal_type_boundaries`, `validation::tests::vocabulary` cover both integer endpoints, zero, booleans, empty/UTF-8 strings/bytes, signed float zero and finite extrema, wrong logical types, forbidden/permitted exceptional floats; `literal_local_encoding` asserts literal uint64-max bytes and binary32 negative-zero bits. Rust typed integer variants cannot represent out-of-range values; wider/signed stand-ins are rejected, and malformed/truncated byte representations fail in `malformed_local_values`. Null has no logical Value representation. |
| SV-03 | `validation::tests::structured` and `structural_mutations` exercise required/optional fields, omission, null stand-in rejection, arrays, unknown retained fields and invalid items; `mandatory_semantics` rejects operation-forbidden fields that generic shape permits; value/codec/schema budget tests cover depth, aggregate nodes/bytes, malicious counts, duplicate fields and malformed descriptors. |
| SV-04 | `metadata_before_body`, `wanted_and_provided_authority`, `real_board_selection_and_staleness` validate metadata and active complete contracts before dispatch; `exact_acceptance_and_stale_offering` binds exact bytes/hash/peer/key and current Board; `expiry_after_dependency_callback` rejects expiry during validation. `durable_binding_and_real_reservation` observes a single real seven-credit hold before execution and immutable consent. Registry, quote and Ledger refusal tests cover absent code, preconditions, changed issuer/terms, maximum, trust and capacity; rejected proposals execute no code or charge. |
| SV-05 | `complete_body_validation_precedes_execution` supplies literal missing/extra bytes, different SHA256 content and wrong type against accepted complete byte metadata; no provider executes or credits settle, and the valid bytes can subsequently be retained. `durable_binding_and_real_reservation` settles fixed seven once; `reject_invalid_completion_without_charge` refuses invalid output and final price eight. Services accepts complete owned logical input; fragment framing and declared body assembly remain Peer Protocol's boundary. |
| SV-06 | `metered_and_negative_completions_settle_real_ledger` uses literal upstream standard definitions and explicit test-only requester rules: max60 holds60 of owned100; billed1201 settles2 and leaves98 available, releasing58; executionError settles2; timeLimitReached settles60. Empty Discovery listIdentities and Public Feeds read outputs settle their literal fixed operation prices2 and1. Pure pricing tests reject metering outside exact Computation v1 and check rounding/overflow/zero-rate limits. These fixtures install no runtime, index or feed provider and claim no full family conformance. |
| SV-07 | `wanted_and_provided_authority` and `requester_only_wanted_result_and_uncertainty` require explicit local input/request authority and finite accounting; fabricated remote metadata is rejected, an absent local provider does not prevent authorized requester work, and accepted roles derive from Board rather than connection direction. No code installation or provider suggestion activation port exists. |
| SV-08 | `paid_evidence_and_private_expectations` sends only a challenge, retains expectations locally, bills complete wrong evidence and returns Fail without refund/trust mutation; missing expectations give Inconclusive. `timeout_without_valid_evidence` gives requester-local zero delta and Inconclusive; late evidence cannot resurrect it. `included_provider_evidence_and_exhaustion`, `concurrent_included_admission` and `local_only_and_reopened_entitlement` bound pre-existing included allowances across concurrent streams/reopen and refuse local-only remote checks before effects. **Global timeout charging remains ambiguous:** `timeout_and_provider_commit_have_distinct_local_credit_views` demonstrates 100 requester-owned versus97 provider-issued after a committed reply is lost; [upstream issue #10](https://github.com/beothorn/CR2SE/issues/10) and [local evidence](../../../CR2SE%20bugs/0007-check-timeout-settlement.md) remain unresolved. Requester-local zero charge is not a cross-peer rollback guarantee. |
| SV-09 | `durable_coordination_fault_matrix` drops all coordinator/participant heaps and reopens only byte images at all eight specified cuts: reservation acknowledgement, reserve-before-prepare, handler prepare, Ledger prepare, before decision, after decision, after one commit and after both before success. Original status/decision delivery produces one resource/charge or explicit uncommitted abort, never re-execution. `fresh_process_recovers_pending_commit` additionally reopens real protected Services/Ledger/family stores in a fresh child process. `unknown_gates_until_recovery`/journal tests, `lifetime_capacity_keeps_results`, `retained_capacity_refuses_without_eviction` and `missing_coordinator_history_cannot_reuse_ledger_key` preserve uncertainty and replay guards; no compaction is offered. Fault cuts simulate acknowledged persistence boundaries, not physical power-loss certification. |
| SV-10 | `requester_explicit_decline`, `cancellation_and_absent_hold_abort`, requester uncertainty and the eight-cut matrix distinguish explicit uncommitted failure from possible commitment. Generic cancellation/timeout cannot erase Commit. `serialized_and_direct_contracts_match` loses a start reply then refuses repeat execution; fresh operation keys identify distinct attempts and there is no automatic work retry. The check-specific timeout reading is qualified under SV-08. |
| SV-11 | `requester_only_and_refusals`, `catalogue_and_capacity` refuse absent exact/optional handlers before selection. `requester_only_wanted_result_and_uncertainty` works with no local provider; `unpublish_before_drain` and `accepted_work_survives_handler_drain` remove new publication/selection while retained accepted work finishes; `missing_recovery_provider_is_explicit` gates restart without required provider code rather than substituting a version. |
| SV-12 | `serialized_and_direct_contracts_match` compares exact quote bytes/hash and identity/key binding, validates full-width uint64/IEEE/body preservation in bounded envelopes, and compares real direct/serialized settlement/output/result bytes; truncated/versioned/foreign-authority calls fail and copied results outlive adapter/coordinator lifetime. Original-key lost-reply status never reruns work. The eight-cut and Linux child tests prove durable result ownership across real process restart. The adapter itself is a local synchronous binding, not an independently implemented process/channel or CR2SE wire codec. |

| Shared case | Evidence and scope |
|---|---|
| AT-C01 | Services-specific Authority, Context and Registry tests reject foreign, dropped, wrong-action/identity, revoked and versioned grants; handler/accounting ports take explicit dependencies. The real public Board/Ledger adapters and test-only byte stores require no private dependency state. |
| AT-C02 | Limits, Value, codec, quote, journal, FileStore and operation tests cover finite capacity before unbounded allocation, exact values, deadline/cancellation, known storage refusal versus uncertain commitment, protected filesystem permissions/symlinks/contention and reduced reopen limits. Terminal history is not evicted to admit work. |
| AT-C03 | Immutable Selection/Quote/Blob and durable operation bindings outlive sessions. Exact handler/effect tokens are owner/key-bound. Whole-image journal CAS, original-key participant status, eight restart cuts and barrier admission/reservation/drain and decision/delivery races preserve one immutable outcome and prevent duplicate execution/charges. |
| AT-C04 | `services_trace_excludes_payloads` captures bounded TRACE output and rejects raw body/control-text markers; production events carry operation descriptions without payloads/keys/expectations. Unsupported families and adjustments fail explicitly. Logging is composed outside protocol code; no network retry, signing, installation, refund or automatic trust-change capability is created. |
| BD-09 | `real_board_selection_and_staleness`, `exact_acceptance_and_stale_offering`, `unpublish_before_drain` plus actual coordinator reservation/settlement bind exact selected definitions, economic roles, raw quote bytes and hash. Peer encoding/current remote Board exchange remains later. |
| LG-13 | Actual Services Coordinator and actual Ledger participate in all eight cuts and the Linux subprocess case; exact reserve/prepare/status tokens and immutable decisions survive loss and duplicate delivery. Third-party fee/net/gross evidence and payment uncertainty are independently tested in `third_party_payment_is_independent`; completed service work is not reversed by unknown/refused payment. |

## Limits and handoff

Default capacities are 4096 handlers, 4096 retained operations, 65536 body buffers,
65536 logical nodes, 64 unaccepted quotes and 16 MiB complete state. History and pins
remain conservative lifetime obligations; lower limits fail reopening, and exhausted
capacity requires operator planning rather than deletion of replay evidence.
The private coordinator payload is version 2; local call/adapter version is 1.
Older alpha payloads are explicitly Unsupported, never silently reset or migrated.

The next module must supply canonical peer encoding/vectors, detached-body
assembly, exact quote encoder/hash association, authenticated original-peer events,
wire success/end/failure classification and separate issuer controls. It must not
retry work on disconnect, assume atomic cross-peer settlement, or advertise the
local serialized adapter as network protocol support. Paid Storage and other family
providers remain separate modules. Revisit issue #10 before claiming globally
uncharged paid-check timeout behavior. Production serve still refuses unsupported
application OPEN requests and installs no application protocol.

## Final verification

On 2026-11-08, formatting, locked warnings-denied all-target Clippy, full locked
unit/integration/rustdoc tests, locked build, overview and Services detailed help,
example run/unit test/help aliases/invalid-argument status and warnings-denied
API documentation passed. Local Markdown links and all 34 ordered checklist/detail
pairs were checked. Production executable-line coverage is 11967/12673 = 94.43%,
using [the repository procedure](../../../docs/coverage.md). Abrupt child-process
tests may leave incomplete LLVM profiles; valid captures are merged without
suppressing test failures or excluding uncovered production code. Build and
coverage artifacts remain ignored. The acceptance limits above remain in force.
