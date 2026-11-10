# Accepted check capabilities

Incoming is created only by valid provider acceptance and durable reservation.
association reads the original trusted requester/connection/stream binding;
invocation borrows the immutable original contract, issuer, exact quote, price and
response interval. It contains no expected value and grants no early execution.

Outgoing is created only after requester reservation and once-only dispatch
marking. association and invocation expose original bindings, accept borrows the
exact ID/hash acceptance, and challenge borrows the defined challenge alone.
These process-local capabilities cannot be fabricated from decoded JSON. Their
lifetime does not own the coordinator journal or accounting hold; dropping one
never proves rollback or releases possible obligations.
