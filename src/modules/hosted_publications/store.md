# Hosted Publications store

Durable, bounded placement storage owned by this module, independent of paid Storage and of Messaging. `Store::volatile(limits)` keeps
everything in memory (with injectable write failures for tests); `Store::open(dir, limits)` keeps owner-only files (directory 0700, files 0600):
one `<placementId>.content` written first, then one `<placementId>.json` record, each replaced atomically. Opening loads and strictly validates
every record: a corrupt, oversized (over 4 MiB), unknown-version, misnamed or inconsistent record, a missing or wrong-sized content for an active
placement, or more records than the bound is `Storage`; nothing is repaired, and an interrupted temporary file is discarded.

`Limits` bounds the number of placements and the content bytes of active ones (`Limit` when exceeded).

- `insert(placement, now)`: never replaces. Under one lock it refuses a repeated placement ID, an active placement of the same publisher with
  the same publication ID, and (for pages and feed posts) an active placement of the same publisher and kind with the same sequence but another
  publication ID, all as `Conflict`, so racing stores cannot both succeed.
- `update(id, change)`: applies a closure to one record under the lock and persists it atomically; identity, metadata and signature cannot change.
- `get`, `record`, `records`, `has_room`, `active_publication(publisher, publication, now)`, `active_of_kind(publisher, kind, now)` (newest first by
  sequence then publication ID, both descending), `active_count(publisher, now)`, `release_content` and `discard` (content first, then record).
