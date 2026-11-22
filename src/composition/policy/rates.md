# Rolling-minute rates

`Rates` counts events per authenticated identity over exactly the last 60 seconds, using sixty one-second
buckets per identity.

- `hit(peer, now, limit)`: count one event unless the last minute already holds `limit` events; returns whether
  the event is within the limit. A refused event is not counted. A clock that moves backwards never loosens
  the limit.
- `count(peer, now)`: events in the last minute. `len()`, `is_empty()`: identities counted.
- At most 4096 identities are counted; the least recently touched is evicted when full.

The node keeps two tables: requests the peer opened (every free read and every paid request is one peer-opened
stream) and failures attributed to the peer (streams that failed or that the peer cancelled).
