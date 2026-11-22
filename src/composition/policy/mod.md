# Peer policy

CR2SE leaves trust, credit bootstrap, quotas, provider selection and redundancy to each
identity. This module isolates those decisions behind six contracts so a deployment can change
them from real usage or simulation without touching protocol code.

| Contract | Responsibility |
| --- | --- |
| [Contracts](contracts.md) | The six decision traits, their inputs and outputs |
| [Parameters](params.md) | Validated `policy.json` with finite ranges and conservative defaults |
| [Observations](observations.md) | Bounded per-peer facts that decisions read |
| [Configured](configured.md) | Implementation of every contract from one parameter set |

Callers: node composition (admission, quotas, credits, connections) and the replica manager.
Dependencies: identities and plain numbers only. A policy never reads peer-supplied text as an
instruction and never changes a CR2SE price, wire value or failure code. Decision inventory:
[docs/policy/INVENTORY.md](../../../docs/policy/INVENTORY.md). Plan:
[plans/12_Peer_policy/PLAN.md](../../../plans/12_Peer_policy/PLAN.md).
