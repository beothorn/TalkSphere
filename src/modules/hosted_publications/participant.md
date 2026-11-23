# Hosted Publications participant

Durable preparation, idempotent commit, abort and status of accepted effects, for the Services coordinator, which prepares, records a decision, then commits or aborts and asks for
status after any crash instead of repeating work.

- `prepare(key, effect) -> token`: refused with `Unavailable` until recovery has admitted new work. Checks the effect's preconditions without changing anything (a store needs a new
  placement ID, room and no host-rule conflict; a renewal or removal an active placement with the exact expected expiry) and locks everything the effect touches: its placement,
  and for a store also its publication ID and, for pages and feed posts, its sequence slot, so two operations that would conflict can never both prepare (`Conflict`). The effect is
  journaled with a random token. Preparing the same effect again returns the same token; a different effect under the key is a `Conflict`.
- `commit(key, token)` / `commit_at(key, token, at)`: applies the effect at most once through the provider engine (at the coordinator's retained decision time `at`, so a delayed
  delivery or a restart reproduces the same acceptance and expiry) and journals the outcome; committing again returns the retained outcome. Applying is idempotent, so a crash between
  applying and journaling is repaired by committing again. A failure to apply, including a refused release credit issuance, leaves the effect prepared.
- `abort(key, token)`: releases a prepared effect and its locks; absent or already aborted is a no-op; a committed operation is a `Conflict`.
- `status(key)`: `Absent`, `Prepared(token)`, `Committed(outcome)` or `Aborted`. `prepared()` lists operations awaiting a decision. `stage_query` journals a read-only operation's request.
