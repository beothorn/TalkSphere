# Cryptography local API contracts

These are in-process interfaces, not working algorithms or serialized messages.
All operations use exact bytes and must enforce bounded input before expensive
work. Unimplemented operations fail with `Unavailable`; none may fabricate output.
Private capabilities need no diagnostic formatting, cloning or byte extraction.
Concrete protected types and cleanup implementations arrive in step 3.

## Values and requests

`KeyPair` returns `public_key` (32 canonical bytes) and `private_key` (an owned,
purpose-specific capability whose provider releases its secret when dropped).
It has no automatic serialization or diagnostic representation.

`RecipientCredential` contains an untrusted `identity_id`, `signing_public_key`
and `encryption_public_key` (32 bytes each), and a 64-byte `binding_signature`.
Possessing these public values grants no private authority. The ID is
SHA-256(0x01 || 0x01 || signing public key); no running Identity module is needed
to verify that formula. Verification is deferred to its planned implementation.

`EncryptRequest` borrows `plaintext` and `recipient` for the call and carries
validated `limits`. There is no external associated-data input for this envelope.
`DecryptRequest` borrows the complete `object` and local `private_key`, and
carries expected `identity_id`, `encryption_public_key` and `limits`. These
expected public values come from the authorized local caller, not the envelope.
Borrowing does not transfer or consume persistent private authority.

## Primitive interfaces

`Hashing.sha256(bytes)` returns a 32-byte digest over exact bounded bytes or a
resource/dependency error. Ordered incremental hashing will extend this interface
when implemented; the scaffold does not concatenate or normalize input.

`Signatures` defines a protected `SigningKey` capability:

- `generate_signing_key()` returns an independently generated Ed25519 key pair;
  entropy/key-store failures release partially created material.
- `sign(key, message)` borrows authorized signing authority and returns an ordinary
  Ed25519 64-byte signature over exact caller-defined bytes, or denied access,
  resource limit or dependency failure.
- `verify(public_key, message, signature)` returns success only for a valid
  signature; malformed inputs, rejected signatures and dependency/resource
  failures remain distinct. Verification grants no trust or permission.

`KeyAgreement` defines separate `PersistentKey`, `EphemeralKey` and `SharedSecret`
capabilities. Providers must keep signing, persistent and ephemeral authority
separate even though interfaces alone cannot enforce a provider's internal policy.

- `generate_persistent_key()` generates an independent X25519 pair; Identity
  remains responsible for storing it and coordinating persistent installation.
- `generate_ephemeral_key()` generates fresh single-use X25519 material.
- `agree_ephemeral(key, remote_public_key)` consumes ephemeral authority and
  returns a protected shared secret or rejects invalid/all-zero results, denied
  access or unavailable dependencies, releasing consumed temporary material.
- `agree_persistent(key, remote_public_key)` has the same validation requirements
  but borrows persistent authority so it remains usable for other objects.

No raw shared result may be used directly as an encryption key. HKDF, AEAD and
Transport integrity APIs will be added alongside their focused implementation
steps; their existing Transport port is not replaced by this scaffold.

## Generic envelope interface

`IdentityEncryption` defines a protected local `PersistentKey` capability.
`encrypt(request)` validates ID/key binding, checks limits, generates fresh
ephemeral material and nonce, then returns owned complete envelope bytes.
`decrypt(request)` validates format, sizes and intended recipient, performs
agreement/derivation and authenticates the complete header and ciphertext before
returning owned plaintext. It must never release provisional plaintext.

Any failure returns a typed error and no partial result. Each operation cleans up
transient secrets on every exit. Self-encryption uses the same operation. Valid
decryption does not authenticate a sender, establish trust or prove content safe.
Neither interface persists credentials or defines Messaging or chunk formats.
