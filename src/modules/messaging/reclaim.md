# Reclaim

Frees the resources of placements that no longer owe anything.

- `Participant::reclaim()`: one pass at the current time. It expires placements whose time has come, releases the payload of expired and removed placements and forgets expired placement records entirely, freeing their slots. It keeps pending and recovered placements until they expire (recovery retries and status answers need the payload and receipt) and keeps removal tombstones so an exact removal retry is answered for the ledger's lifetime.
- A placement held by a prepared operation is skipped until the coordinator decides; new operations cannot prepare on a placement while it is being reclaimed.
- `Report`: how many placements were expired, payloads released, records dropped and placements skipped as locked.
- Errors: `Unavailable` before recovery has completed or when the clock fails; `Storage` when recording an expiry or forgetting a record fails. Every step is idempotent, so running the pass again completes it.
- `Store::discard(id)` removes a placement's payload and then its record; unknown placements are a no-op.
