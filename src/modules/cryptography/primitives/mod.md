# Primitive operations

`hash` exposes bounded SHA-256 over exact bytes and ordered chunks. Other
primitives are introduced in their focused implementation steps. These operations
require no network, trust, Ledger or Identity runtime.

`keys` generates independent Ed25519 and persistent/ephemeral X25519 pairs.

`signature` signs exact bytes and verifies with an explicitly documented strict
canonical verification profile adopted at upstream revision d40b6278.

`agreement` performs consuming ephemeral and borrowed persistent X25519 with
mandatory all-zero rejection.

`kdf` extracts protected PRKs and expands exact construction-specific keys/prefixes.

`aead` seals bounded units and releases plaintext only after authentication.

`integrity` constructs and verifies empty-plaintext Transport frame tags.

[verification_key](verification_key.md) validates canonical non-small-order public
points under the updated CR2SE profile. It permits mixed-order points and performs
no identity hashing or possession proof.
