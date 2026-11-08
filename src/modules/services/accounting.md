# Accounting participant adapter

Accounting is the narrow Services port for original-key reserve, exact-effect
prepare, recorded commit/abort, durable status and authoritative recovery. Calls
preserve deadline/cancellation and return typed failures including Unknown.
Handlers never receive this port. Composition owns its authority.

Ledger.new retains the real Ledger participant, a separately issued Ledger
Coordinate grant and explicit requester/third-party acceptance policy. It never
creates an administrative grant or accesses Ledger private state. Operation keys
preserve requester/connection/stream bytes across independent contracts.

Opaque Ledger preparation tokens are recovered via exact idempotent reserve and
prepare calls using the retained key, terms, price and effect. No private token
fields are forged or serialized. Commit/abort require the coordinator's original
durable decision; callers must not infer abort from timeout. Structured Ledger
errors map one-for-one, preserving insufficient funds and commitment uncertainty.
