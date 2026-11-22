# Peer policy handoff

Peer policy is complete for the finite Linux Storage profile. Evidence: [acceptance](../../../docs/policy/ACCEPTANCE.md); plan:
[plans/12_Peer_policy/PLAN.md](../../../plans/12_Peer_policy/PLAN.md); parameters: [reference](../../../docs/policy/PARAMETERS.md).

## What exists

- Six decision contracts ([contracts](contracts.md)) driven by one validated `policy.json` ([parameters](params.md)); defaults reproduce
  the minimum node, and a [phone-fleet profile](../../../docs/policy/phone-fleet.policy.json) is recommended by the simulations.
- Per-peer observations (bounded, durable, advisory), rolling-minute rates, a registry of live connections by peer, and a client profile
  that keeps contacts connected with pings and bounded reconnects.
- Admission, per-identity quotas, finite bootstrap grants, trust steps and wanted Storage applied by the node ([runtime](../node/runtime.md),
  [owners](../node/owners.md), [wanted](../node/wanted.md)).
- A replica manager for hosted data: sealed items, placement through the wanted offering, staggered checks and renewals, expiry, and the
  CR2SE redundancy states ([replicas](../node/replicas/mod.md)).
- A deterministic simulation of the real policy against a modelled fleet ([simulation](../node/sim/mod.md)).

## Contracts for Messaging, hosted pages and feeds

- **Quotas and rates are service-agnostic.** The node counts every stream a peer opens and every stream it fails, so a new service gets
  per-identity limits without code. A service-specific bound (for example per-sender placements) belongs in `Quota::limits`.
- **Release credits** for Messaging removal are `Credit::release(cumulative_payments, minimum)`: bounded, deterministic, parameterized by
  `credit.releasePercent`.
- **Redundancy** applies to any item that has independent placements: the book's counting rules (confirmed only, distinct providers, ended
  copies count for nothing), `report` and the planner take only items and the clock. Messaging placements and hosted publications should
  reuse them rather than copy them; extracting the book from Storage-specific fields is the first step of the hosted-content plan.
- **Trust and credits.** `Owners::observe(peer, event)` is the single entry for verified observations; `Owners::bootstrap` the single entry
  for first grants. Call them from new service handlers instead of touching the Ledger.
- **Reaching phones.** `Link::connect(peer, None, ...)` returns the live connection whichever side opened it; use it for every
  server-initiated request. Peers that never listen are the normal case.
- **Control.** New operator commands go through `control/commands.rs`; maintenance reuses `Handler::peer_operation` so every paid
  operation is journaled and bounded.

## Not done, in order of importance for the next milestones

1. Recovery of an item from replicas (retrieve, open, verify the plaintext hash) when the node's own sealed copy is lost.
2. A placement rule that excludes or deprioritizes holders that failed checks.
3. An upstream decision on the `retryAfterSeconds` proposal, then rate refusals instead of connection close and requester back-off.
4. Automatic acquisition of the holders' credits that maintenance spends.
