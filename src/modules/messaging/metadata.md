# Message metadata

`Metadata` is the signed, provider-visible description: message ID, sender and recipient identities, the sender-claimed creation time
(not a trusted clock), encoding, exact payload size (at least one) and payload SHA-256. `SignedMessage` adds the sender's 32-byte
public key, the 64-byte signature and the payload bytes. Encryption protects the payload only; every provider sees the metadata. The
debug form shows only the payload length.
