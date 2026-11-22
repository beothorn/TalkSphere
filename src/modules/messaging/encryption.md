# Encrypted Messaging payload

`cr2se.identity-encrypted.v1`. The stored payload is `0x01 || recipientEncryptionKey(32) || ephemeralKey(32) || nonce(24) || ciphertext+tag`, at least 105 bytes and exactly 105 plus
the plaintext length. The message key is HKDF-SHA-256 over the X25519 shared secret with salt `SHA-256(metadataBytes)` and info `CR2SE-MESSAGE-PAYLOAD-KEY-V1`; XChaCha20-Poly1305
authenticates `metadataBytes = ASCII("CR2SE-MESSAGE-ENCRYPTION-METADATA-V1") || messageId || senderId || recipientId || u64be(senderCreatedAt)`. The signature (step 8) covers
this stored representation, the ciphertext, not the plaintext.

- `metadata_bytes(metadata)`: the (non-secret) authenticated data.
- `encrypt(plaintext, metadata, recipient_key, entropy)`: fresh ephemeral key and nonce from secure entropy (missing entropy is `Unavailable`, never a fallback). `recipient_key` must already be a
  verified recipient X25519 key (step 11 validates the Board credential). An empty plaintext or a low-order (all-zero shared secret) key is `Invalid`.
- `encrypt_with(plaintext, metadata, recipient_key, ephemeral, nonce)`: the whole construction with explicit material, so independent vectors can be reproduced.
- `decrypt(payload, metadata, own_key, own_public_key)`: after the caller verified the sender signature and payload hash; checks the version, that the embedded recipient key is the one whose private half is held,
  derives the key and authenticates. Any failure is `Invalid` and returns no plaintext.

Evidence: MS-14 against the independent PyNaCl vector in the Cryptography fixtures (identical payload bytes) plus a wrong key, changed metadata (each bound field), tampered tag or ciphertext, another version and truncation. This is not the generic identity envelope and is never substituted for it.

## `PersistentAgreement`

A holder of the recipient's persistent private key that agrees with a remote ephemeral key and returns only a protected shared
secret. A directly held secret implements it for tests; [`IdentityAgreement`](identity.md) implements it over a protected Identity.
`decrypt` now takes this holder instead of a raw secret; its checks and failures are unchanged.
