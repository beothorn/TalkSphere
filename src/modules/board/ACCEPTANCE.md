# Board acceptance evidence

Behavior baseline: [CR2SE d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
Upstream was rechecked during planning and execution on 2026-11-06: six later
website/workflow/introductory commits change none of the reviewed protocol files.
Tests below implement [BD-01–BD-10](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/acceptanceTests/README.md)
and applicable [AT-C01–AT-C04](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/AcceptanceTests.md#common-contract-cases)
through direct public contracts. Run `cargo test --locked modules::board`,
`cargo test --locked modules::services`, `cargo test --locked --test board_cli`
and, on Linux, `cargo test --locked --test board_persistence`.

## Required suite assertions

| Case | Executed evidence | Scope |
|---|---|---|
| BD-01 | `offering::tests::identity_validation`, `definition::tests::definitions_match_exact_fields`, `acceptance_tests::bd01_bd03_bd10_multiple_ids_and_array_order` | Empty/provided/wanted/populated Boards, complete matching definitions, separate schemas, exact local reads |
| BD-02 | `document::tests::exact_source_and_invalid_syntax`, `parsing_budgets`, `acceptance_tests::bd02_bd03_bd05_rejection_rows`, `terms::tests::economic_ranges_and_metadata` | Missing/wrong root fields, unsupported version, UTF-8/syntax/trailing/duplicate rejection, escaped duplicate names, exact values beyond binary64 and through U64_MAX |
| BD-03 | `identity_validation`, `bd02_bd03_bd05_rejection_rows`, `bd01_bd03_bd10_multiple_ids_and_array_order` | Required fields, version boundaries, duplicate IDs in each/across lists; no first/last wins; multiple IDs sharing one exact contract |
| BD-04 | `terms::tests::economic_ranges_and_metadata`, `acceptance_tests::bd04_check_price_matrix`, `offering::tests::mathematical_integer_spellings_are_exact`, `semantics::tests::capability_checks_are_explicit` | Main/check positive uint64 domains, all specified invalid numeric/type rows, exclusive representations, model/name and embedded-schema rules; exact checked arithmetic; injected known-model overflow |
| BD-05 | `selection::tests::work_roles_and_contract_selection`, `scenarios::remote_selection_requires_direct_provenance`, `bd02_bd03_bd05_rejection_rows` | Provided/wanted roles, exact issuer and alias rejection; neither API accepts TCP opener as an input |
| BD-06 | `document::tests::exact_source_and_invalid_syntax`, `semantics::tests::capability_checks_are_explicit`, `acceptance_tests::bd06_bd07_unknown_siblings_and_mismatches`, `privacy_tests::board_trace_excludes_raw_metadata` | Unknown siblings do not hide known selections; required unknown semantics block use; exact unknown metadata forwarding; hints retained without execution or log disclosure |
| BD-07 | `definition::tests::definitions_match_exact_fields`, `bd06_bd07_unknown_siblings_and_mismatches`, `cache_definitions::tests::definition_receipts_never_mix_revisions`, `state::tests::immutable_atomic_revisions_and_owner_scope` | Each identity/schema mismatch, remote races, absent/removed IDs, no version substitution; failed preparation cannot mutate publication |
| BD-08 | `scenarios::concurrent_reads_pin_complete_pairs`, `state::tests::immutable_atomic_revisions_and_owner_scope`, `recovery_tests::uncertain_commit_requires_recovery_without_replay`, `linux::tests::restart_at_each_durable_cut`, integration `acknowledged_snapshot_survives_process_restart` | Barrier-driven reader/writer races, immutable pins, atomic commit cuts, old/new complete restart state, actual new process reopen |
| BD-09 Board layer | `scenarios::repricing_and_removal_preserve_retained_terms` | Price 7 becomes stale after price 9/removal; retained price-7 copied terms survive owner drop; Board never accepts new terms or cancels work |
| BD-09 quote integration | Deferred | Actual quote acceptance, charge/refusal and durable accepted-invocation obligations require Services and Peer Protocol; not marked passed by a test-owned terms copy |
| BD-10 | `cache::tests::provenance_expiry_and_capacity`, `remote_selection_requires_direct_provenance`, `response::tests::exact_response_limits`, `bd01_bd03_bd10_multiple_ids_and_array_order` | Trusted direct vs forwarded provenance, exact complete response bounds, no array priority; reordering leaves explicit selection unchanged |

## Shared boundary assertions

| Case | Evidence | Applicability |
|---|---|---|
| AT-C01 | `authority::tests::grants_are_scoped_and_revocable`, `dropped_issuer_cannot_authorize`, `state::tests::immutable_atomic_revisions_and_owner_scope`, `scenarios::revoked_and_cancelled_operations_have_no_effects` | Direct-call authority, owner/domain isolation and purpose separation; trusted Cache grants are required to supply provenance; no wire pointer/path fields |
| AT-C02 | `config::tests`, `document::tests::parsing_budgets`, `response::tests::exact_response_limits`, `record_codec::tests::private_record_boundaries`, `semantics::tests`, `recovery_tests` | Invalid/finite bounds, exact arithmetic, dependency rejection and unknown commit recovery without replay; resource limits do not masquerade as success |
| AT-C03 | `state::tests`, `scenarios::concurrent_reads_pin_complete_pairs`, retained-revision case below, `recovery_tests`, `linux::tests`, separate-process integration | Immutable complete results, retained-revision admission, revoked future access and acknowledged durable restart state |
| AT-C04 | `document::tests`, `scenarios::revoked_and_cancelled_operations_have_no_effects`, `privacy_tests`, `board_cli` integration | Complete validated output only, cancelled/malformed input, no payload logging or inferred external actions; no hidden optional dependency calls |

The retained-revision test's exact name is
`scenarios::unpinned_current_does_not_exhaust_single_revision_budget`.
Already returned public bytes are owned results, not revocable authority; issuer
revocation blocks future operations rather than erasing the caller's copies.
Unsupported local contract versions have no serialized negotiation path here:
this direct binding is compile-time contract version 1. Unknown private record
versions fail magic validation. No serialized local adapter is supplied or claimed.

Provider check outcomes, refunds, spending, actual service invocation and remote
payment uncertainty are inapplicable to Board alone. Board returns metadata and
terms; Services/Ledger own those operations. Peer control exchange framing is not
implemented by the metadata response-size helper.

## Fixtures, policies and limits of evidence

[Board test support](test_support/README.md) uses complete specification-derived
literal documents, valid public IDs from the pinned Transport vector and independent
price literals. There is no upstream standalone Board vector file. Schema tests
use the pinned Services vocabulary. Tests exercising explicit service capabilities
install only a test-owned custom contract; production never advertises that fixture.

Real Identity text parsing is used. Local filesystem checks use the real Linux
adapter, owned temporary directories and descriptor-safe operations. Failure cuts
and unknown-acknowledgement stores are injected platform observations, not physical
power-loss tests. `tests/board_persistence.rs` reopens actual acknowledged data in
a separate process. This proves local persistence, not cross-implementation peer
interoperability or separately deployed Board IPC.

TalkSphere strictly rejects malformed Boards. Unknown but structurally valid
semantics remain discoverable; unsupported local providers/requesters cannot publish
through preparation. Cache lifetimes, capacities, strict extra-definition rejection
and conservative custom-contract comparison are documented local policy. Board
version 1 is a schema selector, not a revision field. Family pricing arithmetic is
a trusted capability port; no real Storage or other family pricing provider is
claimed by the injected overflow fixture.

Open upstream reports remain [#8](https://github.com/beothorn/CR2SE/issues/8)
(duplicate-member pseudocode) and [#9](https://github.com/beothorn/CR2SE/issues/9)
(precondition pseudocode/namespace grammar). The normative duplicate and nonempty
rules are enforced. Unknown precondition spellings are never silently satisfied;
no precise namespace grammar is invented before upstream clarification. No upstream
fix is marked adopted without verification.

CLI tests separately verify help independent of logging, exact command syntax,
format-versus-publishability output, oversized/malformed files, publisher mismatch
and corrupt-store preservation. Inspection prints intentional public JSON; operational
TRACE excludes raw metadata. No production Board publish CLI bypasses real capability
composition. Completing this module does not demonstrate the minimum useful node.

## Final handoff audit

Step 24 removes the unavailable scaffold in favor of concrete Provider/Cache
operations and separates Services schema errors from Board internals. Additional
regressions cover schema node bounds without a Board decoder, mathematical version
spellings in contract comparison, conservative unknown contract metadata, conflicting
same-version definitions, withdrawn capabilities, unpinned cache replacement and
private record envelope arithmetic/declared-length rejection. Complete API examples
run as rustdoc tests. Updated public companions describe the resulting contracts.

Quote/payment integration remains the deferred BD-09 row above. Ledger is next;
this module does not register a fake service or establish minimum-node conformance.

`scenarios::known_contract_cannot_be_redefined_by_schema` verifies that schema
structural validity cannot redefine a known service/version: the explicit trusted
contract validator is required in addition to type and identifier recognition.

Final verification on 2026-11-06: formatting, warnings-denied Clippy, locked
unit/integration/doctests, build, API docs, all 26 detailed help spellings and the
documented offline validation example pass. Production line coverage: 6752/7017 = 96.22%
Coverage follows [the production procedure](../../../docs/coverage.md), excluding
test-only code and merging valid captures without suppressing test failures.
