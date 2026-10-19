# Encrypt for a recipient

`encrypt(request, entropy)` validates the credential and object-size policy before
consuming fresh 32-byte ephemeral private material and a fresh 24-byte nonce.
It performs X25519, exact context derivation and AEAD with the entire header as
associated data, then returns one owned header/ciphertext/tag object. There is no
external AAD or sender claim; self-encryption uses the same operation.

Credential, length, entropy, agreement, primitive and allocation failures return
structured errors without partial output. Owned secrets release on every exit,
including nonce failure immediately after ephemeral generation. This follows the
normative cleanup requirement despite the [pseudocode guard gap](../../../../CR2SE%20bugs/0004-secret-cleanup-guards.md).
The caller retains ownership of its input plaintext and must protect it.
