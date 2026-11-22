# Messaging participant

Durable preparation, idempotent commit, abort and status of accepted effects, for the Services coordinator, which prepares, records a decision, then commits or aborts and asks for status after any crash instead of repeating work.

- `prepare(key, effect) -> token`: refused with `Unavailable` until recovery has admitted new work; checks the effect's preconditions without changing anything (a store needs a new placement ID and room, a recovery a pending unexpired placement, a renewal or removal the exact expected expiry); locks the placement
  so no other operation can prepare on it until this one commits or aborts (`Conflict`); journals the effect with a random token. Preparing the same effect again returns the same token; a different effect under the key is a `Conflict`.
- `commit(key, token) -> Outcome`: applies the effect at most once and journals the outcome; committing again returns the retained outcome. Applying is idempotent: an effect already visible in the store (the placement exists, the receipt or removal ID matches, the expiry has passed the target) is recognized,
  so a crash between applying and journaling is repaired by committing again with the original `acceptedAt`. A failure to apply leaves the effect prepared.
- `abort(key, token)`: releases a prepared effect and its lock; absent or already aborted is a no-op; a committed operation is a `Conflict`.
- `status(key)`: `Absent`, `Prepared(token)`, `Committed(outcome)` or `Aborted`. `prepared()` lists operations awaiting a decision, for recovery.
- Timestamps (`acceptedAt`, `recoveredAt`) are taken at the durable commit, never at preparation. Recovery receipts are signed at commit through the signer port.

- `with_release_issuer(issuer)`: installs the issuer used when a removal commits. Committing a `Remove` effect that releases credits first checks (with the fixed decision time and the placement locked) that the removal can be recorded, then issues the credits once under the operation key, then records the removal and releases the payload. An exact repeat of the commit, or crash repair after the removal was recorded, issues again idempotently, so a settlement interrupted between its records is completed exactly once. A removal that releases credits without an installed issuer is `Unsupported`; a refused issuance leaves the placement pending and the effect prepared.

- `commit_at(key, token, at)`: like `commit` but applying the effect at the coordinator's retained decision time `at`, so delayed delivery or a restart reproduces the same acceptance, expiry and receipt times.
- `receipt_at(record, input, at)`: the provider-signed recovery receipt for a decision time (deterministic signing makes finalization and commit agree). `prepared_effect(key)`: the effect journaled as prepared under a key.
