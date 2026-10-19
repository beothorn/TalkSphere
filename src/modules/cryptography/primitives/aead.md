# Authenticated encryption units

`seal(key, nonce, plaintext, aad, maximum_bytes)` takes a protected Content key,
exact 24-byte nonce and unchanged byte strings, returning ciphertext plus a 16-byte
tag. `open(key, nonce, sealed, aad, maximum_bytes)` returns owned plaintext only
after successful authentication. The local byte limit applies independently to
sealed size and AAD; allocation and length arithmetic are checked first.

Malformed nonce or missing tag returns `InvalidInput`, excessive sizes/allocation
failure `ResourceLimit`, and failed tags `AuthenticationFailed`. No partial output
is returned. Private staging buffers erase on failure; successful plaintext is
owned by the authorized caller, who must protect it and treat it as untrusted.

The owning construction assigns nonces and guarantees no key/nonce reuse; the
primitive has no persistent nonce database and does not invent a wire format.
XChaCha20-Poly1305 0.10 is used with alloc only, deliberately excluding its RNG
feature because the explicit entropy port supplies randomness. Backend key objects
zeroize on drop. Fixed upstream AEAD bytes and mutation tests verify behavior.
