# Ordinary Ed25519 signatures

`sign(key, message, maximum_bytes)` borrows signing authority, bounds the message
and returns the exact 64-byte ordinary Ed25519 signature. It uses neither Ed25519ph
nor implicit serialization. A key capability is required; no remote signing
endpoint is provided. Excessive messages return `ResourceLimit`.

`verify(public_key, message, signature, maximum_bytes)` requires 32-byte keys and
64-byte signatures. Malformed values produce `InvalidInput`, excessive messages
`ResourceLimit`, and signature rejection `AuthenticationFailed`. Success proves
only authorization of those bytes and grants no trust or resource permission.

Verification first checks canonical public-key and R encodings and rejects small-order
points, then delegates scalar bounds and the exact uncofactored equation to
ed25519-dalek. This implements the [updated CR2SE profile](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/cryptography/README.md#ed25519-verification-profile),
including mixed-order acceptance when the equation holds. Canonical decoding
errors may return InvalidInput or AuthenticationFailed; every rejection returns
no successful proof. There is no batch API; collections verify each member.

Tests preserve both fixed AUTH signatures and bindings, execute every published
negative signature row and distinguish mixed-order signatures satisfying the
uncofactored equation from those accepted only after clearing the cofactor.
