# Cryptography capability

Cryptography supplies opaque ephemeral, shared, pseudorandom and integrity-key
capabilities. Their owner must release/erase secret material on destruction;
secrets need not be copyable or printable. Every operation may return a typed
DependencyError; failed primitives must never become synthetic success.

| Operation | Inputs and result |
| --- | --- |
| random_nonce | Fresh CSPRNG output, 32 bytes per HELLO |
| ephemeral_key | Fresh X25519 public key and single-use private capability |
| sha256 | Arbitrary bytes to a 32-byte digest |
| verify_signature | Ed25519 public key, message and 64-byte signature; success or rejection |
| agree | Consumes local ephemeral key, uses remote public key, rejects all-zero result |
| extract | Exact Transport salt plus consumed shared secret to HKDF pseudorandom key |
| expand_key | Pseudorandom key and exact label to an opaque 32-byte integrity key |
| expand_nonce_prefix | Pseudorandom key and exact label to a 16-byte prefix |
| tag | Integrity key, 24-byte nonce and header/payload associated data to a 16-byte XChaCha20-Poly1305 tag with empty plaintext |
| verify_tag | Same inputs plus tag; constant-time acceptance/rejection |

Transport owns canonical transcripts, labels and sequencing; this port owns real
primitive correctness and secret lifecycle. Application payloads are authenticated,
not encrypted by this construction.
