# Messaging participant

Durable preparation, idempotent commit, abort and status of accepted effects, for the Services coordinator, which prepares, records a decision, then commits or aborts and asks for status after any crash instead of repeating work.

- `prepare(key, effect) -> token`: refused with `Unavailable` until recovery has admitted new work; checks the effect's preconditions without changing anything (a store needs a new placement ID and room, a recovery a pending unexpired placement, a renewal or removal the exact expected expiry); locks the placement
  so no other operation can prepare on it until this one commits or aborts (`Conflict`); journals the effect with a random token. Preparing the same effect again returns the same token; a different effect under the key is a `Conflict`.
- `commit(key, token) -> Outcome`: applies the effect at most once and journals the outcome; committing again returns the retained outcome. Applying is idempotent: an effect already visible in the store (the placement exists, the receipt or removal ID matches, the expiry has passed the target) is recognized,
  so a crash between applying and journaling is repaired by committing again with the original `acceptedAt`. A failure to apply leaves the effect prepared.
- `abort(key, token)`: releases a prepared effect and its lock; absent or already aborted is a no-op; a committed operation is a `Conflict`.
- `status(key)`: `Absent`, `Prepared(token)`, `Committed(outcome)` or `Aborted`. `prepared()` lists operations awaiting a decision, for recovery.
- Timestamps (`acceptedAt`, `recoveredAt`) are taken at the durable commit, never at preparation. Recovery receipts are signed at commit through the signer port.
