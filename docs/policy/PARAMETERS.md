# Peer policy parameters

Reference for `policy.json`. The same table lives with the code in [params.md](../../src/composition/policy/params.md); step 21 of the [plan](../../plans/12_Peer_policy/PLAN.md) adds tuning guidance from simulations.

`policy.json` sits in the node root next to `node.json`. It is trusted operator input, at most 16384
bytes. It must contain `"version": 1`; every other member is optional and omitted members keep the
conservative defaults that reproduce the minimum node. Unknown members, wrong types, fractions,
negative numbers and out-of-range or inconsistent values are refused before any effect; messages name
the member, never the value. An absent file selects the defaults; an unreadable one is an error.
`node-show` reports `defaults`, `valid` or `invalid`, and `serve --node-root` refuses to start on an
invalid file. Changes take effect at the next start.

| Section.member | Meaning | Default | Range |
| --- | --- | --- | --- |
| `admission.connections` | live connections the node keeps | 64 | 1..4096 |
| `admission.perPeerConnections` | live connections one identity may hold; at most `connections` | 64 | 1..4096 |
| `quota.pending` | simultaneous pending operations per identity | 8 | 1..4096 |
| `quota.requestsPerMinute` | requests per rolling minute, or `null` for no limit | `null` | 1..1000000 |
| `quota.failuresPerMinute` | failed attempts per rolling minute, or `null` | `null` | 1..1000000 |
| `trust.checkPassed`, `contribution` | automatic trust raise, in millionths | 0 | 0..1000000 |
| `trust.checkFailed`, `failure` | automatic trust drop, in millionths | 0 | 0..1000000 |
| `trust.ceiling` | automatic raises never exceed this; required when a raise is set | 0 | 0..1000000 |
| `credit.bootstrapCredits` | credits in a new identity's first grant; 0 disables | 0 | 0..2^64-1 |
| `credit.bootstrapTrust` | trust given with the first grant; required when credits are set | 0 | 0..1000000 |
| `credit.bootstrapBudget` | total credits bootstrap may issue; at least `bootstrapCredits` | 0 | 0..2^64-1 |
| `credit.wanted` | wanted Storage terms (`creditsPerObject`, `maximumObjectBytes`, `retentionSeconds`, all positive) or `null` | `null` | |
| `credit.releasePercent` | early-release share of cumulative payments | 0 | 0..100 |
| `redundancy.target` | independent confirmed copies wanted | 2 | 1..64 |
| `redundancy.checkSeconds` | seconds between checks of one replica | 3600 | 1..31536000 |
| `redundancy.staggerPercent` | per-replica offset as a share of the interval | 10 | 0..90 |
| `redundancy.renewBeforeSeconds` | a replica is renewed once this much retention is left | 600 | 1..31536000 |
| `redundancy.maxSpend` | most credits one check or renewal may cost, in the holder's credits | 100 | 1..2^64-1 |
| `redundancy.retrySeconds` | wait before retrying a check or renewal that could not be completed | 60 | 1..86400 |
| `connection.pingSeconds` | idle ping interval | 30 | 1..3600 |
| `connection.pingTimeoutSeconds` | wait for a pong; at most `pingSeconds` | 10 | 1..3600 |
| `connection.reconnectMaxSeconds` | largest reconnect delay | 60 | 1..3600 |

Automatic trust is off unless a step is set, and a raise requires a nonzero ceiling, so trust never
rises without an operator decision to allow it.
