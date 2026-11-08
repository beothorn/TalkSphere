# Ledger module boundary

Ledger exports local authority and call/operation contracts. Error distinguishes
invalid values, unsupported capability, denied authority, resource exhaustion,
cancellation, conflicting operation reuse, insufficient funds, known storage
failure and possibly committed unknown outcome. Unknown never implies rollback.
No accounting, persistence or peer networking capability is supplied by these
contract declarations; none is advertised by the application.

Unavailable preserves local entropy/protected-dependency loss separately from denied
peer proof and known storage failure. Identity/Cryptography conversion preserves
unsupported, bounds, conflict and outcome-unknown categories. Adapter inputs stay
secret-free in diagnostics; authority is rechecked after blocking reads.
