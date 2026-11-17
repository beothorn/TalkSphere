# Accepted logical removal

`stage` accepts a recovered Linux owner gate, original authenticated accepted
invocation, complete removal input, coherent provider time, optional trusted
elapsed seconds across reboot and value budgets. Services has already accepted
the exact current provider quote and payment authority before using this local
port. Input requires remove plus the retained 32-byte lease ID; factors are empty.
The exact agreed positive price is the selected current offering's removePrice.

Under owner admission it binds original scope and immutable accepted terms,
recovers a matching original result when present, then authorizes ownership and
requires an active unexpired lease. Missing and foreign leases both fail Denied;
inactive leases fail Unavailable. It retains complete output containing remove,
leaseId, removed:true and the accepted price, plus a removal effect guarded by
the current expiration. Bounded result capacity is held before durable staging;
confirmed failures release the provisional hold, while Unknown retains it and
closes admission for recovery.

Existing participant prepare/commit/abort ports serialize renewal/removal and
apply only the coordinator's durable original decision. Staging or preparation
does not end the lease. Acknowledged commitment makes it terminal; later retrieve,
renew and remove attempts fail. Already accepted read pins complete with their
original bytes. Original-key local result recovery is distinct from a fresh
peer invocation: there is no remote cached replay, second charge or refund.
Removal is logical and does not promise physical erasure. Reclamation is step 25.

Shape, terms, role, price, guard, capacity, clock and persistence failures retain
their typed meanings. Unknown is never evidence of abort. Full paid handler and
network/accounting composition remains a later step.

Exercise from the repository root using
`cargo test --locked modules::storage::removal`. See
[requester verification](requester/remove.md) and pinned
[Storage section 9](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#9-remove).
