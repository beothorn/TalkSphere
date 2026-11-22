# Receive

Requester-side acceptance of a recovered message. The recipient authenticates first, deduplicates second and decrypts last.

- `Recipient`: the recipient's identity, its persistent X25519 private key and the matching public key.
- `ProviderKey`: the returning provider's identity and Ed25519 public key (from its Board and authenticated connection).
- `Inbox(maximum)`: an in-memory record of accepted `(senderId, messageId)` pairs with their payload hash, bounded by `maximum`. The persistent catalog belongs to the client journey.
- `accept(recipient, provider, output)`: verifies in order that the sender's key derives to the sender ID, the payload size and SHA-256 hash match and the sender's signature verifies; that the message is addressed to the recipient; and that the recovery receipt carries the recipient's authorization and the provider's signature for exactly this placement and message. Only then is the pair looked up: the same sender, ID and hash is `Duplicate` (a replica from another provider), the same pair with a different hash is `Conflict` (the sender signed two payloads under one ID; nothing is delivered). Encrypted payloads are decrypted last; opaque payloads are returned as received. A new message returns `New` with the content; it is remembered only after decryption succeeded.
- Errors: `Invalid` for any failed check, with nothing remembered; `Conflict` as above; `Limit` when the inbox is full (duplicates are still recognized).
- Message content is never printed.
