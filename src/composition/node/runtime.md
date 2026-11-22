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
