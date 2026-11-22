# Messaging ports

`Clock::now` gives the provider's Unix seconds or `Unavailable`. `Entropy::fill` fills identifier bytes from a secure source or fails;
no identifier is ever derived from a fallback. `Signer` exposes the provider identity, its 32-byte Ed25519 public key and `sign(bytes)`;
it is used only for recovery receipts. Composition supplies platform implementations, tests deterministic ones. Implementations must
not block indefinitely or log secrets.

## ReleasePolicy

`release(cumulativePayments, minimum)` returns the early-release credits the node's policy offers for removing a placement. The module bounds the answer by the payments itself. `NoRelease` always answers zero.

## ReleaseIssuer

`issue(key, recipient, credits)` issues early-release credits to the sender once per Services operation key. Repeating the exact call succeeds without issuing again; the same key with different values is `Conflict`. `Insufficient`/`Limit` mean the issuer cannot issue them; `Storage`/`Unknown` mean the record may or may not be durable.

## PendingLimits

`per_sender(sender)` and `per_recipient(recipient)` give the most pending placements one identity may hold or have waiting at this provider. `FixedLimits` applies the same two numbers to everyone (default 1024 each).
