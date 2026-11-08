# Ledger state owner and administration

Ledger owns one local identity, trusted Authority, positive Limits and one
Persistence port, serialized behind a single writer. Initialize requires Administer
and an absent store, then durably installs empty accounting with operator-supplied
finite issuance/internal budgets. Open requires Read and an existing complete
identity-bound canonical schema; it never creates missing data or repairs corruption.

View requires Read and returns copied issuer-separated values. Budgets returns
remaining issuance/internal capacity. Set trust changes exact local policy only.
Grant requires Administer, a nonself peer and positive amount; it consumes finite
issuance and checks recipient overflow. Revoke subtracts recognized credits without
replenishing lifetime issuance authority. Set owned is explicit local reconciliation,
not a remote query side effect. Every successful change is durable before visibility.

Mutation clones complete state under serialization, validates all bounds, repeats
caller checks against the actual image size and commits before publication. Storage
failure preserves old state. Unknown gates reads and writes until Coordinate-authorized
recover loads/validates the possibly committed image without repeating a mutation.
Corrupt/missing recovery state keeps the gate. No method changes another peer's
ledger, executes services or opens sockets. Reservations constrain
administration to preserve accepted holds and incoming balance headroom.
