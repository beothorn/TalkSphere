# Scoped private operations

Identity.sign(handle, Sign capability, message bytes) checks that the handle belongs
to this provider and the capability matches its issuer, identity, purpose, lifetime
and byte budget. It then enforces the configured signing-byte limit and returns a
complete 64-byte Ed25519 signature over the exact supplied bytes. No hashing,
serialization, prefix, generic challenge or ID-only authentication exchange is added.
The calling protocol owns domain separation, role, freshness and replay protection.
An empty message is allowed by the primitive; the caller must apply its own protocol.

Foreign/revoked/expired/wrong-identity authority returns AccessDenied. Exceeding the
capability or provider budget returns ResourceLimit. Private material stays inside
the loaded handle and Cryptography. The fixed-size native signing backend is
infallible after authorization and bounds checks; unavailable/denied storage prevents
loading a handle in the first place. There is no fictitious remote-signing dependency
to fault-inject. Successful signing grants neither trust nor an authenticated session.
