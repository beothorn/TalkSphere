# Peer Protocol acceptance evidence

Reviewed on2026-11-14 against revision
[d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f),
including the [specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md),
[input/output contracts](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/inputs),
[pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/pseudocode/README.md),
[PP-01–PP-13](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/acceptanceTests/README.md)
and [independent wire vectors](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/pseudocode/TestVectors.md).
Final upstream comparison found the same six website/workflow/introduction-only
commits after this revision; no protocol file changed. Existing
[issue10](https://github.com/beothorn/CR2SE/issues/10) remains open without comments.
No new confirmed language-independent defect emerged from this audit.

This matrix covers the generic Peer Protocol module and its installed-port
boundaries. It does not assert complete standard-family resource/pricing/signature
implementations, global uncharged paid-check timeouts, cross-implementation
interoperability or the minimum useful node. Conditional family cases and the
upstream question are identified below rather than silently counted as implemented.

From the repository root:

```sh
cargo test --locked modules::peer_protocol
cargo test --locked --test peer_protocol_streams
cargo test --locked --example peer
cargo run --locked --example peer -- run
```

Function names below are executable Cargo test filters. The main suite exercises
real independent Board/Services/Ledger owners under explicit test-owned scopes;
[real-node evidence](../../../docs/peer-protocol/NODES.md) separately identifies
actual TCP, two-/three-node and process/restart scenarios. No local scope assertion
is presented as cryptographic authentication.

## Required wire and exchange cases

| Case | Assertions and executable evidence | Scope |
| --- | --- | --- |
| PP-01 | [record tests](tests/record.rs), especially `wire_vectors_and_all_splits`, compare literal board.get and binary-body bytes across every split, single bytes and coalesced records; outbound/sender tests retain offsets under partial admission. | Independent pinned bytes, not local round trips alone. |
| PP-02 | `invalid_and_maximum_records`, `control_syntax_and_bounds`, `all_message_shapes`, `illegal_order_and_interruption` cover unknown kinds, zero/oversized lengths, truncation, malformed/duplicate/trailing JSON, invalid UTF-8, depth33, missing type and order; legal maximum control/body records pass. `framing_and_protocol_failure_are_stream_scoped`, `unknown_control_tags_never_become_semantic_capability_failures` and real TCP malformed-stream isolation keep unrelated work usable without handler execution. | Wire-invalid framing is PROTOCOL_ERROR; separately configured finite resource exhaustion is a resource refusal. |
| PP-03 | [scalar tests](tests/scalar.rs), `scalar_vectors`, `canonical_nonfinite` and lossless [control tests](tests/control.rs) preserve exact0,9007199254740993,uint64 maximum,int64 minimum and signed-zero bits; reject noncanonical integer/IEEE/NaN forms under independent contract policy. Board/definition document tests and `exact_subdocument_source` retain original exact numeric JSON/source. | No conversion through float or a reserialized quote for authority. |
| PP-04 | `inline_boundaries`, `canonical_base64`, `deterministic_descriptors_and_text`, `strict_descriptor_admission`, `ordered_complete_text`, `poisoned_body_streams`, `body_order_and_inline`, `source_admission_and_chunk_limits` cover0/4096/4097 bytes, multibyte UTF-8, unsigned field/index traversal, IDs, exact lengths, noncanonical Base64, unknown descriptor fields, gaps/overlap/extra/missing/wrong-order bytes and invalid final UTF-8. | Unresolved values remain provisional; ignored extensions confer no application authority. |
| PP-05 | `actual_board_metadata`, `publisher_attribution_and_stale_definitions`, `exact_subdocument_source` and endpoint metadata/control tests bind direct publisher provenance, no bodies, both ENDs, missing-offering failure and no service charge. Real TCP/process tests retrieve direct Board and definition metadata through actual owners. | Forwarded or stale metadata cannot become a fresh direct receipt. |
| PP-06 | `exact_provider_quote_and_scoped_release`, `independent_quote_and_explicit_consent`, `quotes_cannot_change_contract_or_payment_or_fabricate_factors`, `exact_acceptance_and_reservations`, `invalid_or_unfunded_acceptance_has_no_provider_hold` bind raw quote SHA256, ID, original connection/stream, contract/version/description, issuer and separately approved spending. Wrong hashes/IDs, early bodies, stale selection, foreign grants and duplicate acceptance cannot execute or settle. | Exact source is retained; economic changes still need local consent and contract changes need a compatible refreshed definition. |
| PP-07 | `specified_pricing_inputs_have_exact_independent_wire_projections` covers the five mandated Files manifest/download, Storage renewal and Messaging recovery/renewal wire-size fields with exact decimal extrema and missing/extra/type refusals. `removal_release_and_unknown_factors_cannot_change_price_authority` separates removal credits. Installed requester assessment/pricing and final result validation are independently invoked. The real third-party test rejects7+2 under bound8 with no requester hold/provider execution and succeeds under9; non-Computation metering is rejected by requester/Services price tests. | Standard family formulas, resource-size authenticity and operation-specific output consistency remain the installed family's responsibility. These wire-factor tests install no Files/Storage/Messaging provider. Generic content mismatch and envelope-price checks are covered now; downstream families must supply their complete formula/resource suites. |
| PP-08 | `complete_input_markers_gate_effectful_dispatch`, `incomplete_or_malformed_input_never_dispatches`, `only_committed_results_can_be_transmitted`, `complete_result_is_applied_only_after_normal_end`, `partial_wrong_and_abnormal_results_cannot_succeed` cover exact bodies, input.end/result.end, both ENDs, fixed final price and provisional-result privacy. Services independently rejects root output.price/finalPrice disagreement. Real node tests complete detached multibyte work and detached wanted results. | No successful output before normal actual Transport completion and acknowledged local participant commitment; payload confidentiality is not supplied by framing. |
| PP-09 | `exact_codes_and_bounded_diagnostics`, `provider_failure_requires_authoritative_uncommitted_abort`, `remote_failure_waits_for_normal_end_and_cannot_follow_success_marker` preserve codes, discard provisional failure output, require original-owner abort acknowledgement and reject failure after success marker. Result-sender/retry tests retain committed output after lost replies; bridge barrier tests retain late effects. Real connection closure returns Unknown while an independent issuer connection remains usable. | No generic remote job-status, automatic financial retry, inferred rollback or refund. |
| PP-10 | `wanted_invitation_uses_local_policy_and_reversed_roles`, `wanted_refusal_does_not_authorize_input_or_spending`, inline/detached endpoint invitation tests and `authenticated_wanted_bodies_and_included_evidence_complete_between_nodes` bind publisher/requesterA and provider/openerB. Missing policy refuses without chosen input or spending; separate consent remains required. | Actual node evidence complements local role/authority tests. |
| PP-11 | `paid_evidence_is_challenge_only_and_locally_verified`, `check_quote_validation_and_local_consent_fail_closed`, `check_input_terminals_and_marker_deadline_gate_evidence`, `late_missing_or_provider_claimed_evidence_cannot_charge`, `local_only_checks_and_entropy_failure_are_explicit`, `included_allowance_tracks_complete_and_uncertain_responses` preserve public challenge-only evidence, local private expectations, exact allowances, original clocks and conservative incomplete/unknown delivery. Real nodes exercise paid and included checks. `cached_retry_returns_original_output_and_charges_at_most_once`, `cached_results_require_original_caller_and_all_request_fields`, `retry_keeps_uncertainty_and_rejects_cross_owner_permits` and endpoint explicit retries bind the narrow original-owner cached recovery/removal exception without new quote/body/charge. | Remote check resources/entitlements are explicit custom substitutes, not Discovery/Storage implementations. Messaging uses its complete pinned definition but unverified fixture signatures and substituted resources, documented [here](test_support/messaging-fixtures.md). Paid global lost-response charging remains issue10. |
| PP-12 | `caller_balances_and_protected_transfer_results`, `query_completion_keeps_original_issuer_provenance`, `encrypted_claim_wire_redemption_and_owner_replay`, `claim_dependency_and_completion_failures_are_explicit`, `exact_fee_consent_and_original_stored_retry` and transfer rejection tests cover exact authenticated caller scopes, independent controls/END, no Services dispatch,297-byte authenticated claim decryption,121-byte signed orders,32-byte keys,64-byte signatures, exact fee/ID acceptance and no commitment on early END. Reply preflight tests refuse undersized capacity before effects. Real process issuer restart returns historical signed transfers without requoting or duplicate debit. | Real Ed25519 and authenticated generic claim encryption are used. Installed protected decryption is required; absent keys are explicitly unsupported. No admin wire operation exists. |
| PP-13 | `independent_issuer_settlement_applies_exact_payment_once`, `forwarded_changed_and_unreachable_issuer_remain_unsettled`, `real_nodes_settle_third_party_work_and_preserve_interrupted_control_uncertainty` bind independently authenticated original-issuer evidence, exact gross/fee/net, separate invocation/issuer connection instances and single-use payment identity. Resource tests include `small_detached_bodies_are_charged_against_aggregate_reference_capacity`, which verifies legal one-byte descriptors cannot evade shared reference limits. Rate, backpressure, slow sibling, per-peer reconnect admission and bounded TRACE tests cover finite metadata/control load without free execution or unbounded queues. | Actual three-node work and both owners' settlement complement explicit refused/unreachable/changed-fee and replay cases. Work remains committed independently of unsettled payment. |

## Shared boundaries and related module handoffs

| Contract | Evidence |
| --- | --- |
| AT-C01: explicit dependencies and authority | Peer Authority/Context tests distinguish version, deadline, cancellation, identity/action and foreign/revoked issuers. Endpoint composition accepts only public owners and local policies. Real Transport authentication supplies scope; payload IDs never create keys, grants, installed handlers or signing authority. |
| AT-C02: bounds and failure paths | Fixed wire maxima, preallocation header/JSON preflight, logical values, per-peer/shared admission, worker queued/running/unread capacity, output/backpressure, original deadlines/rate windows and finite owner journals are tested. Financial control replies are preflighted before commitment, while postcommit transmission uncertainty remains Unknown. |
| AT-C03: immutable ownership and recovery | Exact quote/source/hash and opaque owner permits outlive transient streams. Concurrent requester acceptance and independent issuer confirmations apply once; accepted work survives drain/cancellation and retains late worker cleanup. Services/Ledger durable recovery evidence remains in their own matrices; the peer process test adds protected issuer restart and signed replay. |
| AT-C04: observability and absent capabilities | `trace_diagnostics_omit_payloads_and_claim_envelopes` bounds captured TRACE and excludes raw values/envelopes; original scopes/timing and structured typed errors remain observable. Unsupported families, check rules, wanted policy and protected claims refuse explicitly; no absent standard service is advertised. |
| BD-09 | Actual Board selection/current checks plus peer exact quote consent and reservation prove the remote metadata-to-work boundary; staleness and changed contracts cannot silently replace definitions or accepted terms. |
| LG-13 and Services SV-12 | Peer codecs remain distinct from the local Services binary adapter. Actual owners perform reservation/prepare/decision/commit and retain final output/payment state. Independent issuer observations reconcile exact original books, while module-owned restart/decision recovery remains covered by [Services](../services/ACCEPTANCE.md) and [Ledger](../ledger/ACCEPTANCE.md). |

## Independent vectors and declared limits

[Fixture provenance](test_support/fixtures/README.md) records pinned upstream Git
blob43b66be73740988d42a27ae5c1f5fa3919196238, literal record/value examples and an
independently calculated whitespace-bearing source hash. The hash fixture is a
local association oracle, not a complete wire-valid quote. Complete valid quotes
also retain their raw source in actual issuer/requester tests. Literal field/number
oracles supplement generic round trips; actual Transport/crypto vectors remain
in their own modules. Same-implementation TCP does not replace independent vectors.

The [runtime guide](../../../docs/peer-protocol/RUNTIME.md) records exact example
budgets and initialization order. The [node evidence](../../../docs/peer-protocol/NODES.md)
distinguishes volatile custom service owners from persistent issuer records and
real process boundaries. Production composition must restore every real family
participant and its independent durable state before resuming accepted decisions.
No generic remote resume protocol has been invented.

The module's generic responsibilities are implemented and verified with these
scope labels. Full paid Storage is the next module; optional family conformance
and the minimum useful node milestone remain distinct roadmap work. Upstream
issue10 limits global paid-check timeout settlement claims until clarified.

## Final verification

All38 detailed plan steps are complete under the scope above. Formatting, strict
all-target Clippy, locked tests/build, all examples, documented overview/detailed
help and exit statuses, actual custom-node run, warning-free API docs and local
Markdown/companion-contract checks pass. The documented production-only coverage
procedure measured20026/21376 executable lines =93.68% on2026-11-14.
