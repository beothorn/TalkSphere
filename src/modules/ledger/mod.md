# Ledger module boundary

Ledger exports local authority and call/operation contracts. Error distinguishes
invalid values, unsupported capability, denied authority, resource exhaustion,
cancellation, conflicting operation reuse, insufficient funds, known storage
failure and possibly committed unknown outcome. Unknown never implies rollback.
No accounting, persistence or peer networking capability is supplied by these
contract declarations; none is advertised by the application.
