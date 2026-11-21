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
