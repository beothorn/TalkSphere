# Independent key generation

`signing_key(entropy)`, `persistent_key(entropy)` and `ephemeral_key(entropy)` each
consume a separate 32-byte secure entropy request. They return canonical public
bytes and an opaque signing, persistent agreement or ephemeral agreement secret.
Failure propagates without synthetic output; all temporary buffers are zeroizing.

Ed25519 public derivation uses ed25519-dalek 2.2 with default zeroization; X25519
uses x25519-dalek 2.0 with static_secrets and zeroize. Cargo.lock pins versions.
Scalar clamping belongs to the standard backend; keys are never converted between
algorithms. Persistent generation does not install credentials or change Identity
state. Authorized composition owns rollback/cleanup when composing several calls.
