# Local-policy decisions: CR2SE and TalkSphere

CR2SE leaves these decisions to each identity's local policy. This inventory lists
each one, the CR2SE rule that bounds it, and where TalkSphere decides it today
(baseline: minimum node, commit `0143d3d`). The peer policy plan
([plans/12_Peer_policy/PLAN.md](../../plans/12_Peer_policy/PLAN.md)) moves each
"today" entry behind a policy contract in `src/composition/policy/` with its
parameter in `policy.json`. Upstream references use
[CR2SE bc53df4](https://github.com/beothorn/CR2SE/tree/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16).

| # | Decision | CR2SE bound | Today in TalkSphere | Policy contract |
|---|---|---|---|---|
| 1 | Who may connect | [Transport](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/transport/README.md) authenticates any identity; policy is local | The listener admits every authenticated peer up to the transport connection limit (64); no per-identity rule | `Admission` |
| 2 | Free control reads | Board and definition reads are bounded control operations | Served to any authenticated peer, with only the bridge's global `controls_per_second` limit | `Admission`, `Quota` |
| 3 | Initial trust | [Unknown identities start at zero trust](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/ledger/README.md#trust-and-bootstrap) | Zero; changed only by `node-ctl trust` | `Trust` |
| 4 | Trust adjustment | Local, asymmetric, `[0,1]`; no compulsion | Manual `node-ctl trust` only | `Trust` |
| 5 | Bootstrap credits | Finite grant or claim, never hidden unlimited balances | Manual `node-ctl grant` per peer; `credits.issuance` is the total budget | `Credit` |
| 6 | Earning credits | Wanted offerings; the requester decides to accept | Library support exists (Storage wanted policy) but the node installs none (`wanted: None`) | `Credit` |
| 7 | Per-identity quotas | Providers apply sender/requester quotas by local policy | One constant `capacity.perOwner` for simultaneous operations; no byte, lease or rate quota per identity | `Quota` |
| 8 | Failed-attempt rate limits | Providers must rate-limit failed identity proof and probing | Only the global bridge control rate | `Quota` |
| 9 | Provider selection | Requester's choice | The operator names the contact on every `node-ctl` command | `Redundancy` |
| 10 | Replica target | At least two independent agreements to call data redundant | None; one lease is one copy chosen by the operator | `Redundancy` |
| 11 | Independence rule | Distinct provider identities at least | Not tracked | `Redundancy` |
| 12 | Check cadence | Staggered where practical | Manual `node-ctl check` | `Redundancy` |
| 13 | Renewal and repair | Replace copies when confirmed copies fall below target | Manual `node-ctl renew`; no repair | `Redundancy` |
| 14 | Release-credit amount | Provider chooses, bounded by cumulative payments | Not applicable until Messaging | `Credit` |
| 15 | Connection upkeep | `PING` timeout and outstanding pings are local policy | No ping policy, no idle policy, no reconnect; contacts are tried once | `Connection` |
| 16 | Reaching an outbound-only peer | Either peer may open streams on one connection | Only the connecting side initiates requests | `Connection` |
| 17 | Capacity refusal | `resource_limit` with no charge | Provider capacity (`capacity.*`) refuses before work | `Quota` |

## Notes

- Every decision keeps CR2SE's exact values and bounds. A policy chooses among
  allowed behaviors; it never changes a price, a wire value or a failure code.
- Policy never reads peer-supplied text as an instruction. Inputs are recorded
  observations, operator configuration and the clock.
- Items 6, 14 and the Messaging parts attach to later milestones; their contract
  exists from this milestone so those modules need no new policy plumbing.
