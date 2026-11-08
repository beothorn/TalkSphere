# Services call contracts

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

Limits bound registered handlers, durable operations, body references, logical value
nodes, unaccepted quotes and serialized state bytes. Every budget is positive;
state framing must leave 64 bytes of arithmetic headroom. Defaults are 4096
handlers/operations, 65536 bodies/nodes, 64 quotes and 16777216 state bytes.
These local defaults are not protocol maxima. Accepted obligations cannot be
evicted merely to admit new work.
