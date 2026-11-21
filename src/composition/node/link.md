# Node command link

The node loop alone owns Transport and the Peer Protocol bridge. Control workers reach it only
through a `Link`: a command queue plus a wake signal.

- `Link::new(signal)` returns the worker handle and the receiver the loop drains.
- `connect(peer, address, timeout)` returns a live authenticated session to the pinned peer,
  asking the loop to connect when none exists. It fails promptly when the node is stopping or the
  peer does not authenticate within the timeout.
- `open(connection, factory, timeout)` asks the loop to open one locally authorized exchange.
  Only the admission result returns here; the exchange outcome arrives on the plan's own channel.
- `retain_cache` keeps the direct Board and definition cache fetched for a peer's session;
  `forget` drops a session so the next request reconnects; `closed` (loop only) drops the sessions
  of a closed connection.

Every wait has a deadline, so a lost reply can never hang the node. A session is a connection this
node opened; incoming connections are served by the provider handler, not through the link.
