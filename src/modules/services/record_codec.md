# Private protected-image envelope

These private Linux persistence helpers do not define a public peer format.
Maximum validates Limits and adds48 bytes of checked framing overhead. Encode
returns a complete bounded image: ASCII TSSERVC1, owner identity32, big-endian
uint64 payload length, then exact payload bytes. Decode verifies magic, complete
length, configured maximum and absence of trailing data before copying payload
into an owner-associated Record. Invalid framing returns Invalid, excessive size
or arithmetic returns Limit. This outer framing version is separate from the
coordinator payload's private schema version2 and local call version1.
