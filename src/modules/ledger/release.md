# Release

Idempotent issuance of early-release credits, bound to one Services operation key.

- `issue_release(context, key, peer, amount)`: with Coordinate authority, issues `amount` of the ledger's finite local issuance to `peer` as one durable record under `key`. Repeating the exact call (same peer and amount), for example after a crash or lost reply, succeeds and changes nothing. The same `key` with a different peer or amount is `Conflict`. A zero amount or the local identity as peer is `Invalid`; exhausted issuance is `Insufficient` and a peer with no room is `Limit`, both with no change. `Storage` and `Unknown` behave as for every Ledger commit.
- `release_of(context, key)`: what `key` already issued, with Read authority; it changes nothing.
- Records are kept for the ledger's lifetime and bounded by the operations limit; a full set refuses new keys but still answers old ones. They appear in the persisted state only when present, so older images stay valid.
- This is separate from `grant`, which is an administrative, non-idempotent call.
