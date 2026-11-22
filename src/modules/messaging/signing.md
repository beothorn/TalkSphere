# Message signing

`signing_bytes(metadata, payload)` is exactly `ASCII("CR2SE-MESSAGE-V1") || messageId || senderId || recipientId || u64be(senderCreatedAt) || encodingCode ||
u64be(payloadSize) || payloadHash || payload` (domain without terminator; code 0x00 opaque, 0x01 encrypted). The signature is ordinary Ed25519 over those
bytes and covers the stored representation, which for an encrypted payload is the ciphertext.

- `sign_message(signer, recipient, message_id, created_at, encoding, payload)`: a sender builds the metadata (size and SHA-256 from the payload) and signs. Empty or oversized
  payloads are `Invalid`; a signer that cannot sign is `Unavailable`.
- `verify_message(message, authenticated_sender)`: in the specified order, (1) the sender ID derives from the public key, (2) it equals the authenticated requester when one
  is given, (3) the payload is nonempty and its size and SHA-256 match, (4) the signature verifies over the exact bytes. The first failure is `Invalid`. A recipient
  repeats the checks after recovery with no authenticated sender.

Evidence: MS-01 against the independently generated vector (`fixtures/vectors.json`): same bytes, same signature, verification succeeds, and mutating any metadata field, the payload, the
key or the signature is rejected.
