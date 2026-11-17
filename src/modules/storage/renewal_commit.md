# Accepted renewal staging and commitment

`stage` takes a recovered Linux owner gate, original authenticated accepted
invocation, complete input, coherent provider time, optional trusted elapsed
seconds across reboot, and value budgets. Services must have accepted the exact
quote and reserved payment authority. This operation is a local family execution
port, not a peer endpoint or an independent payment coordinator.

Within serialized owner admission it binds original scope, accepted terms,
quote, input and factors, checks retained original results, authorizes the lease
again, and checks the current guard, clock, limits, exact price and sole size
factor. It creates a complete `renew` output with lease ID, previous expiration,
new expiration and price, plus a guarded renewal effect preserving every other
lease field. Only bounded local values are allocated; no object is uploaded.
Result capacity and one pending operation are reserved before durable staging.
Confirmed failed staging releases the provisional hold. Unknown persistence
keeps the hold and closes admission for original coordinator recovery.

The returned Completion is staged, not successful paid output. Existing Gate
prepare/commit/abort ports apply only Services' durable decision. Preparation
rechecks the exact guarded mutation against authoritative state and excludes
conflicting prepared renewal/removal. Commitment applies it once and retains
original result/status. At most one renewal for an expected expiry succeeds;
new stale attempts fail. A timely staged renewal is not resampled at the old
expiration during participant recovery. Only original-key local recovery returns
saved output; there is no new remote cached-retry operation or second extension.

Failures preserve original authorization, guard, clock, value/capacity, price and
persistence errors. Conflict rejects changed original key reuse and stale or
conflicting mutations. Unknown must never be interpreted as abort or rollback.
Complete paid handler/payment/network integration remains a later plan step.

From the repository root run `cargo test --locked modules::storage::renewal_commit`.
See [requester renewal validation](requester/renew.md) and the pinned
[Storage renewal contract](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#8-renew).
