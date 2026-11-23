# Hosted Publications acceptance evidence

Status: complete for the planned module scope (plan steps 1-24 of [milestone 15](../../../plans/15_Static_pages/PLAN.md)) except **HP-09**, which belongs to the Public Feeds module
(milestone 16). Implemented and tested: signing and the independent vectors, verification, the restricted envelope, terms and exact prices, the durable store and lifecycle, every operation, the Services
codec, definition, provider adapter and requester client, crash recovery across Services, Ledger and hosted files, the Peer Protocol requester intents, the best-known version and equivocation rules and an
authenticated four-node exercise with a publisher that only dials out. Nothing is advertised on a Board by default.

The behavior follows the CR2SE Hosted Publications proposal ([CR2SE 53f1245](https://github.com/beothorn/CR2SE/tree/53f12451859e0ad207ade9b8a23be20ceb404495), proposed in
[beothorn/CR2SE#14](https://github.com/beothorn/CR2SE/issues/14), **not yet accepted upstream**); the vectors in `fixtures/vectors.md` were generated independently by the upstream script. One local CR2SE commit
(the Peer Protocol `contentSize` pricing input for hosted get and renew) is not pushed and is recorded in [REQUIREMENTS.md](REQUIREMENTS.md).

| Case | Executed evidence (all in `cargo test --locked --lib modules::hosted_publications` unless noted) | Remaining scope |
| --- | --- | --- |
| HP-01 | `signing::tests::signing_reproduces_the_independent_vectors` (V1, V2, V3 signing bytes, SHA-256 and signatures), `every_mutation_changes_the_signing_bytes`, `verify::tests::*` reject every mutation. | |
| HP-02 | `provider::tests::a_store_creates_the_priced_durable_placement` (price 17, 1000/1010), `get_returns_the_exact_signed_publication`, `store::tests::native_stores_survive_a_restart`, `service::crash_tests`. | |
| HP-03 | `provider::tests::rejections_change_nothing`, `verify::tests::signed_rule_violations_are_rejected`, `store::tests::duplicate_publications_and_sequences_conflict_atomically`, `quotas_and_tables_are_finite`. | |
| HP-04 | `provider::tests::restricted_items_are_hidden_without_a_trace`, `restricted_gets_and_lost_content`. | |
| HP-05 | `provider::tests::list_orders_pages_and_charges` (order, exclusive cursor, `more`, empty page charged, invalid requests). | The equal-sequence pair of the case cannot be stored under the specification's own same-sequence rule (REQUIREMENTS gap 3). |
| HP-06 | `provider::tests::get_returns_the_exact_signed_publication`, `get_failures_are_bounded_and_concealed`; `service::coordinator_tests::the_client_prices_exactly_and_refuses_bad_factors`. | |
| HP-07 | `envelope::tests::the_vector_envelope_is_reproduced_and_opens` (the 295-byte V3 envelope), `damaged_or_altered_envelopes_fail_without_plaintext`, `every_audience_member_opens_and_strangers_do_not`. | |
| HP-08 | `modules::pages::hosted::tests::*` and `manifest::tests::the_vector_manifest_is_reproduced` (V2). | |
| HP-09 | Not in this milestone: feed-post content checks belong to Public Feeds. The V3 restricted feed-post envelope and signature are reproduced. | Milestone 16. |
| HP-10 | `latest::tests::*`: the greatest verified sequence, equivocation reported and never chosen, best-known and never proven latest; `provider::tests::quotas_and_tables_are_finite` and the store rule reject a second page at one sequence per host. | |
| HP-11 | `provider::publisher::tests::renewal_extends_from_the_exact_expectation`, `renewal_commits_are_idempotent`, `lifecycle::tests::*`. | |
| HP-12 | `provider::publisher::tests::removal_releases_credits_once_and_replays`, `quotes_are_bounded_and_commits_repair_crashes`; `recovery::tests::a_crashed_removal_completes_once`; `service::crash_tests` (credits issued once with a lost Ledger acknowledgement); the endpoint test's retry over a new stream. | |
| HP-13 | `provider::publisher::tests::check_serves_one_byte_to_the_publisher_only`, `check::tests::*`, `service::coordinator_tests::byte_checks_are_paid_bounded_and_judged_privately`, and the endpoint test `a_paid_check_is_judged_with_the_publishers_private_expectation` (real endpoints, exact charge, pass/fail/inconclusive). | |
| HP-14 | `provider::publisher::tests::only_the_publisher_may_change_or_check`. | |
| HP-15 | `standard::tests::*` (the pinned definition), `service::tests::*`, `service::coordinator_tests::every_operation_runs_through_services_with_exact_charges`; `cargo run --locked --example pages -- run` stores independent replicas at two hosts (separate placement IDs) and reads the page from one host alone. | |

## Added evidence

- `peer_protocol::engine::endpoint::tests::hosted_publications` runs the real family over three independent module stacks and real endpoints: every operation at its exact price, a retry over a new stream, a paid check and a
  quote above the requester's own price refused.
- `cargo test --locked --example pages` and `cargo run --locked --example pages -- run` (see the [exercise guide](../../../docs/pages/EXERCISE.md)).
- A full lifecycle through the real Services coordinator, Board and Ledger (`service::coordinator_tests`): store 17, list 1, get 2, renew 8, remove 2 releasing 12 credits.

## Limits

- One placement is one copy at one host; redundancy is the publisher's choice. Hosts can withhold or serve an older version but cannot forge content; absence proves nothing.
- The upstream proposal is unaccepted and may change; recheck against the accepted revision. Three questions are recorded for upstream in REQUIREMENTS.md (HP-05's seed, the sizes of restricted manifest entries and the
  Peer Protocol pricing input).
- The exercise runs four nodes in one OS process; host process crash and restart is covered by the module crash tests, not by the example.
- Installing the family in the node composition is part of the client journey.
