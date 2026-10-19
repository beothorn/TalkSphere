# Real Cryptography to Transport adapter

`TransportCrypto.new(provider)` takes an already configured real Provider and
implements Transport's Cryptography port without starting I/O. Associated secrets
are purpose-scoped ephemeral, shared, PRK and integrity capabilities. Nonces and
public ephemeral keys are fresh; ephemeral agreement consumes its private key.

The adapter delegates SHA-256, ordinary Ed25519 verification, safe X25519, exact
HKDF labels, 16-byte nonce prefixes and empty-plaintext integrity tags to real
primitives. Transport still owns transcript construction, AUTH ordering, direction
assignment, counters and replay state. Provider limits must accommodate the intended
Transport maximum frame/AAD and handshake sizes.

Invalid input and resource excess map to InvalidInput, authentication rejection to
VerificationFailed, access refusal to Denied and unavailable dependencies to
Unavailable. No underlying input or secrets are included. Identity persistence and
authenticated server startup remain separate; this adapter never supplies fake
identity proofs. Cryptography's documented strict Ed25519-policy caveat applies.
