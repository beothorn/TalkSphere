# Hosted Publications effects and outcomes

`Effect` is the exact state change of one accepted operation: `Query` (list, get: nothing changes; it keeps the encoded request so a lost staged result is produced again
after a restart), `Store` (placement ID chosen at preparation, the verified publication, retention, price), `Renew` (placement, expected expiry, seconds, latest allowed expiry,
price) and `Remove` (the verified removal request, the agreed release credits and the removal price). `Outcome` is what committing it produced: `Query`, `Stored` (placement,
publication, `acceptedAt` and `expiresAt` assigned at the durable commit, price), `Renewed` and `Removed`. Both have a strict durable JSON encoding (`encode`, `decode`; corruption is
`Storage`), which is the participant's own format and never a peer wire format. `placement()` names the placement an effect concerns.
