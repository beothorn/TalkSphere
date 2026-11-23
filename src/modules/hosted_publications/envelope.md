# Hosted Publications envelope

Restricted content is stored as `0x01 || ephemeralPublicKey(32) || contentNonce(24) || u32be(entry count) || entries || contentCiphertext`,
where each entry is `readerEncryptionKey(32) || wrapNonce(24) || wrappedContentKey(48)`. The header is 61 bytes, every entry 104 bytes and
the ciphertext at least 16 bytes. Entries follow the audience order.

`metadataBytes` is ASCII `CR2SE-PUBLICATION-ENCRYPTION-METADATA-V1` followed by publication ID, publisher ID, kind code, sequence and
claimed time. For each reader key `R`: the shared secret is X25519(ephemeral private key, `R`); the wrap key is HKDF-SHA-256 of it with salt
SHA-256(metadataBytes) and info `CR2SE-PUBLICATION-KEY-WRAP-V1`; the wrapped key is XChaCha20-Poly1305 of the 32-byte content key
authenticated by `metadataBytes || R`. The content is XChaCha20-Poly1305 of the plaintext under the content key authenticated by
`metadataBytes`.

- `seal(plaintext, metadata, readerKeys, entropy)` draws the ephemeral key, content key and nonces from a secure source and returns the
  envelope; `Unavailable` without randomness, `Invalid` for empty or oversized plaintext, no readers, or a reader key giving an all-zero
  shared secret. `seal_with` takes every random value explicitly so the independent vector can be reproduced.
- `checkStructure(content, audienceCount)` is the keyless host check: version 1, entry count equal to the audience count and at least
  header, entries and a tag.
- `open(content, metadata, ownKey, ownPublicKey)` finds the entry carrying `ownPublicKey`, unwraps the content key through the holder's
  agreement (the private key never leaves it) and decrypts. Any failure is `Invalid` and no plaintext is returned.

A reader in the audience can copy the plaintext, every member sees every audience key, and hosts see the audience IDs.

`minimumBytes(entries)` is the smallest valid envelope (header, entries and a tag), so a restricted publication whose declared size is below it is refused from the header.
