# Static Pages acceptance evidence

Status: complete for the planned scope of [milestone 15](../../../plans/15_Static_pages/PLAN.md): canonical paths, media types, the hosted page manifest and assembly, the publisher-served `cr2se.page` version 1 service
(terms, typed get, local check, resource map, definition, Services provider adapter and requester client, Peer Protocol requester intent) and its interplay with Hosted Publications. Nothing is advertised on a Board by default.

Behavior follows [CR2SE Static Pages at 73edf2d](https://github.com/beothorn/CR2SE/tree/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/pages) and, for hosted pages, the unaccepted Hosted Publications proposal
([beothorn/CR2SE#14](https://github.com/beothorn/CR2SE/issues/14)).

| Case | Executed evidence (`cargo test --locked --lib modules::pages` unless noted) | Remaining scope |
| --- | --- | --- |
| PG-01 | `service::store::tests::local_publication_replaces_and_removes` (root rule: missing, not HTML, over the limit), `service::tests::an_unusable_root_stops_service`; peers cannot publish, edit or list. | |
| PG-02 | `service::tests::a_get_returns_the_exact_output_at_the_fixed_price` (the SHA-256 of `abc`, price 4); `service::coordinator_tests::every_get_is_charged_once_at_the_fixed_price` (real Services, Board and Ledger). | |
| PG-03 | `path::tests::*` (every valid and invalid path, case and normalization sensitivity), `service::tests::invalid_paths_are_refused_before_any_access` (the resource source is never read, nothing staged or charged). | |
| PG-04 | `media::tests::*`. | |
| PG-05 | `service::tests::bounds_and_empty_resources` (bound below, above the offering, exact, empty resource with the empty SHA-256 and the fixed price), `service::terms::tests::*`. | |
| PG-06 | `service::tests::a_missing_resource_is_a_failure_not_a_substitute`, `service::store::tests::symbolic_links_and_oversized_files_are_refused` (confined directory source). | |
| PG-07 | `service::tests::staged_results_are_pinned_snapshots`, `service::store::tests::local_publication_replaces_and_removes` (a pinned snapshot keeps its bytes); the endpoint test `a_new_get_after_a_local_change_sees_the_new_snapshot`. | |
| PG-08 | `service::tests::staged_results_are_pinned_snapshots` (two visitors see the same content selected only by path); `service::terms::tests` accept different prices and limits. | Two concurrent offerings of one identity are not run through the coordinator. |
| PG-09 | `service::check::tests::*` and `service::tests::the_client_checks_results_locally` (pass, every failing condition, inconclusive; no resource signature). | |
| PG-10 | `service::tests::staging_survives_a_restart` (journaled request rebuilds a staged result, a lost reply is answered from the committed status, abort), failed gets cost nothing in the coordinator and endpoint tests. | The Services SV-09 crash matrix is run for Hosted Publications, not for the stateless Page service. |
| PG-11 | `service::coordinator_tests::every_get_is_charged_once_at_the_fixed_price` and the endpoint test `every_get_completes_over_real_endpoints_at_the_fixed_price`: only the root is paid until an asset is explicitly requested and charged; no implied freshness. | |
| PG-12 | The provider and client depend only on Services handler contracts, the codec and the hash; no Files, Discovery, Computation or Storage module is imported. | |

The hosted page cases of Hosted Publications (HP-08, HP-10) are in [its acceptance record](../hosted_publications/ACCEPTANCE.md).

## Added evidence

- `peer_protocol::engine::endpoint::tests::pages`: the real Page family over two independent stacks and real endpoints at the fixed price, a quote above the visitor's own price refused.
- `cargo run --locked --example pages -- run` assembles hosted pages (public and restricted) from two hosts.

## Limits

- A pass of the local check proves only self-consistency: no truth, safety or authorship, and no transferable proof.
- Content is untrusted: page scripts and instructions have no authority to invoke services, publish, sign or spend.
- Installing the families in the node composition, and a publisher or reader command over a node, belong to the client journey plan.
