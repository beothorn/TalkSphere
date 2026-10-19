# Ordinary Ed25519 signatures

`sign(key, message, maximum_bytes)` borrows signing authority, bounds the message
and returns the exact 64-byte ordinary Ed25519 signature. It uses neither Ed25519ph
nor implicit serialization. A key capability is required; no remote signing
endpoint is provided. Excessive messages return `ResourceLimit`.

`verify(public_key, message, signature, maximum_bytes)` requires 32-byte keys and
64-byte signatures. Malformed values produce `InvalidInput`, excessive messages
`ResourceLimit`, and signature rejection `AuthenticationFailed`. Success proves
only authorization of those bytes and grants no trust or resource permission.

The backend's strict verification rejects weak points. CR2SE does not yet fix
this edge-case acceptance profile: see [bug 0003](../../../../CR2SE%20bugs/0003-ed25519-weak-point-validation.md).
This is documented local rejection policy, not an invented normative wire rule.
Fixed upstream AUTH vectors match for both roles; mutation tests reject changes.
