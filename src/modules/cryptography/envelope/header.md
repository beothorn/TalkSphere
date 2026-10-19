# Generic identity-envelope header

`HEADER_BYTES` is 132 and `CONTEXT_BYTES` is 124. Header holds recipient_id,
recipient_key and ephemeral_key (32 bytes each), nonce (24 bytes), and an unsigned
64-bit sealed_length counting ciphertext plus tag. These fields are public,
untrusted metadata; decoding does not authenticate them.

`encode()` writes suite bytes 01 01 01 01, then the three fields at offsets
4/36/68, nonce at 100 and big-endian sealed length at 124. Length below 16 fails.
`decode(bytes)` requires exactly 132 bytes, the four supported selectors and sealed
length at least 16. Invalid values return `InvalidInput` without fallback, body
allocation or text conversion. U64_MAX is representable header metadata; later
complete-object parsing must reject arithmetic overflow or resource excess.
