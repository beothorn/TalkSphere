# Logical Storage resource admission

Usage counts retained logical bytes, lease slots, pending operations and result
bytes for one trusted original operation key. Quota.new validates positive finite
Limits. Its authoritative owner must restore all durable holds before exposing
new admission after restart; the index itself is process-local.

reserve atomically admits a nonzero Usage under the original operation key.
Identical repeats consume no extra capacity; changed reuse returns Conflict.
Checked aggregate sums and per-requester pending counts enforce all limits.
Zero new usage is Invalid; excessive/overflowing totals are Limit; poisoned locks
are Unavailable. reserve does not execute work or authorize financial effects.

transition atomically replaces an existing hold after the owner's authoritative
durable transition, without a capacity gap. Zero usage releases the hold. A failed
transition preserves prior capacity; missing keys are Unavailable. Committed
leases can retain bytes/slots while reducing their pending operations to zero.

There is no automatic release on timeout, session loss or dropped observers.
The owner must retain prepared holds through uncertainty and apply only known
commit/abort decisions. This index neither supplies durability nor proves local
caller authority: it belongs behind the owner's authorized serialized boundary.
