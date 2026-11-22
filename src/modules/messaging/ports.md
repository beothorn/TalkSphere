# Messaging ports

`Clock::now` gives the provider's Unix seconds or `Unavailable`. `Entropy::fill` fills identifier bytes from a secure source or fails;
no identifier is ever derived from a fallback. `Signer` exposes the provider identity, its 32-byte Ed25519 public key and `sign(bytes)`;
it is used only for recovery receipts. Composition supplies platform implementations, tests deterministic ones. Implementations must
not block indefinitely or log secrets.

## ReleasePolicy

`release(cumulativePayments, minimum)` returns the early-release credits the node's policy offers for removing a placement. The module bounds the answer by the payments itself. `NoRelease` always answers zero.
