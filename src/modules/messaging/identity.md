# Identity-backed payload agreement (`identity.rs`)

Lets a recipient whose private keys live in a protected Identity decrypt Messaging payloads without the key leaving Identity.

## `IdentityAgreement`

A bundle of three borrowed values:

- `identity`: the Identity provider holding the protected keys.
- `handle`: the loaded handle of the recipient identity.
- `capability`: a `Decrypt` capability scoped to that identity, with a byte budget of at least 32.

It implements `PersistentAgreement`: given the sender's 32-byte ephemeral public key it asks Identity for the X25519 agreement and
returns only a protected shared secret. Any denial (foreign handle, wrong scope, budget, low-order key) is reported as `Invalid`.
Key derivation (HKDF over the metadata hash) and authentication stay in `encryption`; Identity learns nothing about Messaging.
