# Peer policy acceptance evidence

Baseline: [CR2SE bc53df4](https://github.com/beothorn/CR2SE/tree/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16), the revision
the whole project follows. Sources: [trust and bootstrap](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/ledger/README.md#trust-and-bootstrap),
[Redundancy](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/Redundancy.md),
[Peer Protocol failures](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/peer-protocol/README.md) and
[Transport streams and PING](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/transport/README.md).
CR2SE defines no acceptance cases for local policy, so the evidence is TalkSphere's own tests of CR2SE's rules plus the
decisions CR2SE leaves open. Real TalkSphere processes at both ends; no other implementation is involved.

Run: `cargo test --locked --test policy_process` (15 tests, about 30 s), `cargo test --locked composition::policy composition::node`,
and the simulation with `cargo run --locked --release --example policy_sim -- sweep`.

## Matrix

| Rule or decision | Evidence |
| --- | --- |
| Unknown identities start at zero trust; a provider does no paid work for them | `unconfigured_peer_reads_metadata_and_is_refused_paid_work_at_zero_trust`: an unconfigured peer reads the Board and definition, is refused paid work, and nothing is charged |
| Authentication is not trust; bootstrap is finite, explicit and never repeated | `bootstrap_policy_grants_once_per_identity_within_the_budget`: two peers granted, the third refused when the budget is used up, no repeat after a crash and restart; `Owners::bootstrap` records the grant durably before the Ledger changes |
| Trust moves only on verified observations | `trust_follows_verified_contributions_and_checks`: steps up to a ceiling, never above it, down after a failed check; `Observations` and `Configured` unit tests |
| Rate-limit abusive identities without free resource execution | `request_rate_limit_closes_only_the_noisy_peer`, the `Rates` and abuse-experiment unit tests, and the sweep: a limit of 120 a minute refuses about 99 % of flooding and no honest request in the model |
| Bounded admission | `admission_policy_refuses_connections_beyond_the_limit`; `Peers` registry bounded and cleared on close (`live_connections_are_tracked_and_forgotten`) |
| Either peer may open streams on one connection | `server_calls_an_outbound_only_client_over_its_own_connection`: a server buys from a client over the connection the client opened, exact balances, prompt failure after disconnect |
| `PING` timeout and reconnect are local policy | `an_unanswered_ping_closes_the_connection_and_the_client_reconnects` (SIGSTOP freezes the server), `a_client_reconnects_after_the_server_restarts`; schedule unit tests |
| Wanted offerings: the requester decides, finite budget | `a_client_earns_credits_by_fulfilling_wanted_storage`: exact credits, the lease cataloged as a replica, a second contribution refused by the issuance budget without charge, state kept across a restart |
| Redundancy: count only complete successful results, distinct providers, show target and confirmed separately | `a_hosted_item_is_replicated_to_distinct_clients_up_to_the_target`; `Book` and `report` unit tests (a target of one is met without being redundant) |
| Staggered checks and renewals; inconclusive is not proof | `replicas_are_checked_and_renewed_before_they_expire`, `an_unreachable_holder_expires_and_the_item_is_replicated_again`, `a_failed_check_removes_the_replica_and_lowers_trust`; planner unit tests (staggering, bounded passes, retry delay) |
| The whole story | `a_server_keeps_a_replica_set_at_outbound_clients_and_repairs_it`: outbound clients earn exactly 5 credits each, the set reaches `target satisfied`, a holder disappears and its copy expires (`not redundant`), the next offer repairs the set |
| Policy is isolated, validated and inspectable | `Params` unit tests (every range and inconsistency), `policy_file_is_validated_at_inspection_and_startup`, `policy_show_and_explain_report_decisions_and_their_inputs` |
| Tunable from simulation | the [simulation](../../src/composition/node/sim/mod.md) tests (reproducible, each model alone) and [SIMULATIONS.md](SIMULATIONS.md) |

## Final checks

Run from the repository root before this handoff: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked`, `cargo build --locked`, `cargo run --locked -- --help`, `cargo run --locked -- help policy`, `cargo doc --locked --no-deps`
with warnings denied, and the production coverage procedure in [coverage.md](../coverage.md) (result: **92.66 %**, 30079/32462 lines, recorded in [coverage.md](../coverage.md)).

## Known limits

1. **Rate and failure limits close the connection.** CR2SE has no specified refusal for a rate limit, so the node closes the
   offending connection, which turns the outcome of anything else on it into unknown. An upstream proposal (optional
   `retryAfterSeconds` on `resource_limit`) is pushed and reported as [CR2SE#13](https://github.com/beothorn/CR2SE/issues/13); see
   [the quota review](QUOTA_REVIEW.md). The pending-operation quota is one value for all peers because the Storage gate takes one number.
2. **Bootstrap does not price identities.** A new identity gets its own grant while the budget lasts. CR2SE cannot prevent this;
   the budget bounds the loss.
3. **Placement is client-initiated and not selective.** A provider must offer in CR2SE, so the server cannot push a replacement
   to a client; a dropped copy is offered to the next client that fulfils the wanted terms. No rule excludes clients that failed
   checks (the simulation shows dishonest clients are paid in proportion to their share).
4. **The server needs ongoing credit from every holder.** Checks, renewals and retrieval are charged in the holder's credits; the
   operator funds them (`grant`/`own`). Nothing acquires that float automatically.
5. **No recovery path yet.** Items are sealed and replicated and the sealed bytes also stay on the node, but retrieving a replica
   and opening it after the node loses its own copy is not implemented (the client journey's recovery step).
6. **Replicas are sealed with a node data key** (`replica.key`, XChaCha20-Poly1305), not the generic identity envelope, because the
   running node does not expose its identity's decryption authority. Back the key up with the node's identity.
7. **Wanted queue and observations are advisory:** the manual backlog is in memory, observations flush every five seconds, and the
   maintenance loop's operations appear in the control journal.
8. **Simulation figures are model figures.** They run the real policy code against a modelled network, Storage and prices.
9. Linux only, finite profile (objects at most 16384 bytes, Storage only). Messaging, hosted pages and feeds attach to these contracts
   in their own milestones.
