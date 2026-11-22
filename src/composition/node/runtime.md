# Node runtime

`run(layout, server_config, signal, report, control)` runs one node until shutdown.

Startup order is fixed: read `node.json`; load the protected identity (taking its exclusive lock);
open and recover every durable owner; publish the Board; compose Transport; register Peer Protocol
with the authenticated Transport; bind the listener and report `Ready`; attempt configured
contacts; then start local control through the `control` hook. Nothing is reachable from the
network before recovery finishes, and a failing stage stops startup with a message naming it. If the
control hook fails the node releases everything and returns its message.

The loop polls Transport events (each routed to the Peer Protocol adapter), advances bridge work,
answers worker commands and waits on the shared signal. A connection the adapter refuses (for
example over its connection limit) is closed without stopping the node.

On the first shutdown request (SIGINT or SIGTERM) the loop stops admission, fails waiting connects,
refuses new exchanges and lets accepted exchanges finish while connections drain, up to the
configured shutdown time. A repeated request or the deadline forces remaining connections closed and
returns a shutdown error. Control stops after the drain, then Transport, the bridge and the owners
are released and `Stopped` is reported, so the identity lock is free for the next start.

## Admission and observations

Every authenticated connection is recorded as an observation. Connections the peer opened are checked by the
[admission policy](../policy/contracts.md) with the node's and the peer's live connection counts; a refused
connection is closed before it is registered or served. Connections this node chose to open are never refused.
An unconfigured peer is admitted by default and may read the free Board and definition; paid work still needs
the provider's nonzero Ledger trust. Observations are written every 5 seconds and at shutdown.

## Quotas and rate limits

Per-identity limits come from the [quota policy](../policy/contracts.md) and apply before any work or charge.
Every stream a peer opens counts as one request in a rolling minute, so free Board and definition reads and
paid requests share one limit; every stream that fails or that the peer cancels counts as one failure. A peer
that exceeds `requestsPerMinute` or `failuresPerMinute` has the offending connection closed and a refusal is
recorded, and refused events are not counted again. Simultaneous pending operations per requester use the
smaller of `capacity.perOwner` and `quota.pending` (one value for all peers, because the Storage gate takes
one number); a request over it is refused before work with `resource_limit` and no charge. Defaults impose no
rate limit.

## Client profile

`serve --node-root R --contact CONTACT` keeps each contact connected: the first attempt is Transport's one-shot start,
and [upkeep](upkeep.md) retries failed or lost connections with the policy's bounded growing delay. Every live
connection, whichever side opened it, is pinged at `connection.pingSeconds` and closed when a pong is late, so a
silent peer is dropped and the registry never lists a dead connection for long. A contact that already connected to this
node is not dialed again. A node with no `--contact` behaves as before.

## Bootstrap credits

When a peer opens a connection and authenticates, the [credit policy](../policy/contracts.md) may give it one finite
first grant (`credit.bootstrapCredits` with `credit.bootstrapTrust`), once per identity and within the total
`credit.bootstrapBudget`; the Ledger's own issuance budget (`credits.issuance`) still bounds everything. The grant is
recorded durably in the observation store before the Ledger changes, so a crash can lose a grant but never repeat one,
and a restarted node never grants twice. Trust is only raised, never lowered below what the operator set. The peer must
still record the credits it holds and trust the issuer on its own side. Bootstrap is off unless the policy sets credits.
