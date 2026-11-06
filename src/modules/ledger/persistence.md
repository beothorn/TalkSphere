# Ledger durable image contract

Record contains the exact local identity and a complete bounded private payload.
The Ledger owner validates its schema and invariants before visibility. Every
transaction serializes mutation, builds a complete candidate, persists it and only
then publishes success/visible state. Preparation has no partial balance effects.

Persistence has one exclusive writer. Load returns absence only for a genuinely
uninitialized store, not corrupt or unreadable state. Commit replaces the complete
image atomically and succeeds only after required durability synchronization.
Storage failure before commitment preserves the prior image. Unknown may mean the
new image committed; the owner must gate affected operations and recover through
load/status, never retry a new mutation or infer rollback. Atomicity is local only.

Memory is explicitly volatile and copies complete images. It has no filesystem
or crash durability claim. Production Linux persistence is a separate adapter.
