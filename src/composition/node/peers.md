# Live peer registry

`Peers` is a shared, bounded table of authenticated live connections keyed by peer identity, updated by
the node loop and read by workers and policy.

- `add(context, connection)`: record an authenticated connection (either side may have opened it); refused
  when 4096 are tracked or the connection is already recorded.
- `remove(connection)`: forget a closed connection and return its peer.
- `count()`, `count_for(peer)`: live connections in total and for one peer.
- `connection_to(peer)`: the newest live connection to a peer, so the node can open exchanges toward an
  outbound-only client over the connection the client opened.
- `peers()`: distinct connected peers in first-connected order.

The loop records on authentication and removes on close, so the table never outlives a connection. GOAWAY
ends in a close observation and is handled the same way. `node-ctl status` reports `connections` and
`connectedPeers`.
