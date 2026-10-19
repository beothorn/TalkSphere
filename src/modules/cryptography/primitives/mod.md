# Primitive operations

`hash` exposes bounded SHA-256 over exact bytes and ordered chunks. Other
primitives are introduced in their focused implementation steps. These operations
require no network, trust, Ledger or Identity runtime.

`keys` generates independent Ed25519 and persistent/ephemeral X25519 pairs.

`signature` signs exact bytes and verifies with an explicitly documented strict
weak-point rejection policy pending upstream clarification.
