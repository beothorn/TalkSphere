# Connection upkeep

`Upkeep` is the schedule behind the client profile (`serve --node-root --contact ...`). It performs no I/O: the node loop
asks what is due, submits the Transport requests and reports the results back.

- **Reconnect**: each configured contact has an attempt count, a next-attempt time and an in-flight flag.
  `due_connects(now, connected)` returns contacts that are not connected (by any route), have no attempt in flight
  and are past their delay, marking them in flight. `failed(peer, now, delay)` counts the failure and sets the delay;
  `connected(peer)` resets the count. The delay is `Connection::reconnect_delay_seconds(attempts)` from policy: it
  doubles from one second up to `connection.reconnectMaxSeconds`.
- **Ping**: `due_pings(now, interval, live)` returns the live connections that have no ping outstanding and are at least
  one `connection.pingSeconds` interval past their last ping, each with a unique 8-byte token. `ping_finished`
  accepts only answers to pings this schedule sent. A ping that is not answered within `connection.pingTimeoutSeconds`
  finishes with an error and the loop closes that connection, which then follows the reconnect path.
- Closed connections are forgotten, so the schedule never grows with churn.
