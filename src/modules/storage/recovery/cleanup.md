# Bounded terminal resource cleanup

`Gate.collect` takes coherent trusted provider time, optional trusted elapsed
seconds across reboot, a positive maximum physical attempt count, and an optional
exclusive lease-ID cursor. It requires recovered admission. It advances the
persisted logical retention clock and expires active leases at their exact
deadline, except targets of unresolved staged/prepared metadata effects. Those
targets retain their original guard until coordinator resolution; new requests
still check current time and cannot exploit the delayed terminal marker.

It sorts eligible expired/removed IDs after the cursor and attempts at most the
requested number. Metadata and inventory traversal remain bounded by configured
owner image limits. Pending conflicting effects exclude an ID from reclamation.
Every open immutable byte handle, including separately reopened handles and range
clones, retains a shared lifetime marker. Physical deletion waits until all such
pins disappear. Opening/pinning and reclamation synchronize on the same local
registry, closing the race between testing pins and unlinking. Already accepted
reads are never invalidated to free capacity.

Deletion uses the protected owner directory descriptor and confirms the directory
durability barrier before releasing logical byte capacity. Confirmed absence
after an interrupted prior deletion is idempotent. On recovery, capacity reflects
actual retained object entries plus descriptor-only pinned objects. Lease-ID slots,
complete result buffers and original operation/coordinator history remain charged
and retained; cleanup neither compacts history nor reuses IDs. Finite history
limits can still refuse new work and never evict accepted obligations. Unresolved
receiving/orphan inventory remains an explicit owner recovery failure.

`Cleanup.expired` counts newly persisted expirations; `examined` counts terminal
objects attempted; `reclaimed` counts confirmed durable absence, even if already
absent; `pinned` counts deferred live byte lifetimes. `next` contains the last
examined ID only when more eligible IDs remain. Pass next as the following
call's cursor to avoid a pinned object starving later IDs; start a fresh sweep
with no cursor after next is empty. Resetting the cursor also revisits pins
released since a previous batch. No count implies a refund or secure erasure.

Zero batch is Invalid, closed admission Unavailable, and clock/storage failures
retain their meaning. A confirmed deletion failure leaves terminal state and
its capacity hold unchanged; retrying cleanup is safe. Unknown metadata/deletion
persistence closes admission and keeps holds until owner recovery. Neither clock
rollback nor failed deletion can reactivate a terminal lease or erase its result.

Exercise from the repository root:
`cargo test --locked modules::storage::recovery::cleanup`.
The behavior follows pinned [Storage concurrency and removal rules](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#14-concurrency-and-idempotency).
