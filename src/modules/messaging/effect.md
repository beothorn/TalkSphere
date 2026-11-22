# Messaging effects and outcomes

`Effect` is the exact state change of one accepted operation: `Query` (discover, status, check: nothing changes), `Store` (placement ID chosen at preparation, the verified message, retention, price), `Recover` (the verified recovery request and price),
`Renew` (placement, expected expiry, seconds, latest allowed expiry, price) and `Remove` (the verified removal request, the agreed release credits and the removal price). `Outcome` is what committing it produced: `Stored` (placement, message, `acceptedAt` and `expiresAt`
assigned at the durable commit, price), `Recovered` (the recorded receipt), `Renewed`, `Removed` and `Query`. Both have a strict durable JSON encoding (`encode`, `decode`; corruption is `Storage`), which is the participant's own format and never a peer wire format.

- `Query { request }`: a read-only operation (discover, status). It keeps the encoded request so a lost staged result can be produced again after a restart.
