# Placement store

The module's own durable, bounded storage of placements, independent of the paid Storage service. `Store::volatile(limits)` keeps everything in memory (tests, simulations; `fail_writes` injects write failures). `Store::open(dir, limits)` is native: an owner-only directory (0700)
with, per placement, `<id>.payload` and `<id>.json` (0600), each written to a private temporary file, flushed and renamed, payload first.

- `insert(&Placement)`: never replaces (`Conflict` for an existing ID); `Limit` when the placement count or the active payload bytes would exceed the bounds; `Storage` on a failed write, with nothing recorded.
- `update(id, change)`: applies the closure to the record under the store lock and persists atomically; a refusal by the closure or a failed write leaves the previous record. The placement ID, message metadata and signature never change. Unknown: `NotAvailable`.
- `record(id)`, `get(id)` (record and payload; a missing or wrong-sized payload is `Storage`, never returned partly), `records()`.
- `pending_for(recipient, now, after, limit)`: pending, unexpired placements ordered by `(acceptedAt, placementId)`, strictly after an optional pair cursor. `pending_counts(sender, recipient)`: for placement quotas.
- `release_payload(id)`: drop the payload bytes of a placement that no longer owes them; the record stays.
- Opening rebuilds the index and refuses a corrupt, oversized (over 16 KiB), unknown-member or misnamed record, or an active placement whose payload is missing or the wrong size; nothing is repaired. An interrupted temporary file is discarded.

Incomplete uploads create no obligation: a store request holds its payload in memory under a bounded reservation (step 16) and a placement is written only at commit, so there is no durable `receiving` state to recover.
- `discard(id)`: forgets a placement and its payload (payload first, so a crash leaves a valid record without payload); unknown placements are a no-op; `Storage` when the record cannot be removed.
