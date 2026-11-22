# Board encryption credential

The optional top-level Board member `identityEncryption` is `{"version":1,"identityPublicKey","x25519PublicKey","bindingSignature"}`, each string unpadded RFC 4648
Base32 (32, 32 and 64 bytes). It belongs to the Board publisher and is how a sender finds the key to encrypt to.

- `field_value(identity_key, x25519_key, binding)` builds the member to publish. `base32_encode` / `base32_decode` are strict: lowercase is accepted, but whitespace, padding,
  other characters, nonzero unused trailing bits and impossible lengths are `Invalid`.
- `find_recipient_key(board, publisher)`: `Ok(None)` when the member is absent (the lookup path is unavailable; opaque messages still work). Otherwise the key is usable only when the version is 1,
  the Base32 values decode to the exact lengths, the ID derived from `identityPublicKey` equals the expected `publisher`, the binding signature verifies over the X25519 key, and that key
  is contributory (every low-order point is refused). Any failure is `Invalid` and makes only that key unusable: the Board's offerings stay valid, because Board validation ignores the member.
  An unverified key found on a Board is never returned.
- `RecipientKey` carries the verified identity, its Ed25519 key and the X25519 key to hand to [encryption](encryption.md).

Evidence: MS-15 (valid, wrong publisher, bad binding, bad Base32, missing, low-order key, unrelated offerings still usable, opaque message without a credential) and RFC 4648 Base32 vectors.
