# Renewal assessment and pricing

`Context` supplies trusted authenticated peer identity and original key, current
provided offering, serialized owner image, coherent provider time, and optional
trusted elapsed seconds across a boot change. These values come from composition
and the owner, never payload identity or clock fields.

`assess(context, input description, value budgets, maximum work credits)` resolves
bounded complete renewal metadata, authorizes the lease, and rejects inactive or
expired leases before agreeing work. Missing and foreign leases both return
Denied. Unresolved small fields return Unsupported rather than fabricated values.
A stale expected expiration returns Conflict. Positive additional seconds must
fit current duration limits. Both expected-expiry-plus-addition and
original-acceptance-plus-total-limit use checked unsigned arithmetic; the former
must not exceed the latter. Current limits do not shorten an existing lease.

`Renewal.assessment` carries complete known input metadata, the sole public
`sizeBytes` factor copied from the retained lease, original logical input byte
bound, and no provisional resource pin. `price` is the exact work charge and
`expires_at` the proposed Unix-second expiration. Nothing is reserved, charged,
staged or extended by this assessment. Accepted execution must recheck owner
state and the guard under durable serialization.

`price(offering, original input, provider factors, independent lease, budgets)`
performs the same guard and limit checks and requires exactly one factor named
`sizeBytes` with the exact retained uint64 size. Requester callers must use their
own catalog lease, not provider-asserted metadata. It calculates
`max(minimumPrice, ceil(size * additionalSeconds * renewalRate))` using existing
exact rational arithmetic. Current maximumBytes restricts new stores and does
not cap an older lease's renewal size. Even uint64-sized factors require no byte
object allocation. Malformed, extra, missing or forged factors fail; overflow
and configured bounds return Limit; exceeding maximum work credits returns
Insufficient. Services owns issuer fees and acceptance authority.

This follows [Storage section 8](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#8-renew).
From the repository root, exercise it with
`cargo test --locked modules::storage::renewal`.
