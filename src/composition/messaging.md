# Messaging node ports

Production ports for the Messaging module, built from a node's shared protected identity.

## `NodeIdentity`

Wraps a `ProtectedIdentity` (see [identity](identity.md)) so one process-wide credential serves Transport authentication, Messaging
signatures and payload decryption.

- As a Messaging **signer** it reports the identity ID and Ed25519 public key and signs exactly the bytes given (messages, recovery
  requests, receipts). A signing failure is reported as unavailable. If the credential cannot be read, the ID and key read as all zero,
  which no signature verifies against.
- As a payload **agreement holder** it performs X25519 agreement with the sender's ephemeral key through Identity's `Decrypt`
  authority and returns only a protected shared secret. Every refusal is reported as invalid.

## `SystemClock`

Unix seconds from the operating system; unavailable before the epoch.

## `SystemRandom`

Operating-system randomness for identifiers and payload encryption; unavailable when the system cannot provide any. No fallback is
used.
