# Ledger call contracts

Contract version is exactly 1. OperationKey identifies an authenticated requester,
trusted unique connection instance (32 bytes) and stream instance (uint32). It
survives local retries; another connection or stream denotes a new attempt.
Reservation records bind this key to complete immutable accepted terms,
roles, issuer, bound and result/effect references; the key alone is not authority.

Context carries version, borrowed owner-issued grant, absolute monotonic deadline
and cancellation observation. Check rejects unsupported versions, expired or
cancelled calls, wrong domain/identity/action and excess byte budgets before work.
Adapters must repeat checks at effect boundaries and supply authentic contexts.
This does not establish rollback after possible commitment.

Limits names maximum accounts per view, durable operations, lifetime transfer
records, all claims, simultaneous quotes and serialized state bytes. All must be
positive, with room for 64 bytes of framing without overflow. Owners enforce each
budget before admission. Limits never authorize dropping retained obligations.

Default local capacities are 4096 accounts per view and operations, 65536 transfers
and claims, 64 live quotes, and 16777216 serialized state bytes. These are deliberate
local policy, not new peer protocol maxima; reopening with smaller incompatible
limits fails instead of dropping records.
