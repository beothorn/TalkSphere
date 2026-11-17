# Scoped immutable byte ranges

Handle.range validates a positive exact object range by subtraction, then returns
an immutable Range retaining the descriptor without copying bytes. It never
returns the underlying whole-object handle. Invalid rejects zero, overflow-shaped
or out-of-object ranges.

Range.len returns accepted length; is_empty is false for supported ranges. read
takes an offset relative to this range, positive length and independent allocation
maximum. It validates the requested subrange before computing the absolute object
offset, preserving the accepted boundary. Invalid rejects escaped ranges; Limit
bounds allocation; Storage preserves incomplete backing reads.

Clones share immutable descriptor ownership and use positional reads without a
shared mutable cursor. They can finish accepted reads after metadata expiry,
removal or unlink. The owner must retain/reconcile resource quotas and lifetime;
these references authorize neither a new lease operation nor additional spending.

Internal provider result construction can verify a bounded complete-object hash
while collecting only the accepted range in that same scan. Whole-object bytes
outside the range never become public output; short/corrupt backing data fails
before a successful completion is returned.
