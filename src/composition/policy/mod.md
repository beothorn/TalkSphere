# Peer policy

CR2SE leaves trust, credit bootstrap, quotas, provider selection and redundancy to each
identity. This module isolates those decisions behind six contracts so a deployment can change
them from real usage or simulation without touching protocol code.

| Contract | Responsibility |
| --- | --- |
| [Contracts](contracts.md) | The six decision traits, their inputs and outputs |
| [Parameters](params.md) | Validated `policy.json` with finite ranges and conservative defaults |
| [Rates](rates.md) | Bounded rolling-minute counters per identity |
| [Observations](observations.md) | Bounded per-peer facts that decisions read |
| [Configured](configured.md) | Implementation of every contract from one parameter set |

Callers: node composition (admission, quotas, credits, connections) and the replica manager.
Dependencies: identities and plain numbers only. A policy never reads peer-supplied text as an
instruction and never changes a CR2SE price, wire value or failure code. Decision inventory:
[docs/policy/INVENTORY.md](../../../docs/policy/INVENTORY.md). Plan:
[plans/12_Peer_policy/PLAN.md](../../../plans/12_Peer_policy/PLAN.md).

## Status, build and test

All six contracts and their parameters are complete: admission, per-identity quotas and rate limits, trust steps, finite bootstrap
credits, wanted Storage, replica targets with staggered checks and renewals, and connection upkeep are applied by the node
composition. Upstream references: [CR2SE Ledger trust and bootstrap](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/ledger/README.md#trust-and-bootstrap)
and [CR2SE Redundancy](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/Redundancy.md); the
[quota review](../../../docs/policy/QUOTA_REVIEW.md) records the proposal made upstream.

From the repository root: `cargo test --locked composition::policy` for the contracts, parameters, observations and rates;
`cargo test --locked --test policy_process` for real node processes; `cargo run --locked --release --example policy_sim -- help`
for the simulation. Callers: node composition ([runtime](../node/runtime.md), [owners](../node/owners.md), [replicas](../node/replicas/mod.md)).
Policy depends only on identities and plain numbers so another deployment can replace any contract without touching protocol code;
the node composition owns every effect a decision leads to.
