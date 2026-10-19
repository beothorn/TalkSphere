# Cryptography resource policy

`MINIMUM_OBJECT_BYTES` is 148: the generic 132-byte header plus a 16-byte tag.
`ResourceLimits` is an immutable pair of local byte budgets.

- `new(maximum_plaintext_bytes, maximum_object_bytes)` returns validated policy
  or `InvalidInput` when plaintext capacity is zero or object capacity is below
  148. It does not allocate buffers, perform cryptography or start I/O.
- `maximum_plaintext_bytes()` returns the permitted plaintext byte budget.
- `maximum_object_bytes()` returns the complete envelope byte budget.

The budgets are independent, so the object budget can further restrict usable
plaintext capacity. Empty plaintext is allowed under a positive budget. These
limits do not change the wire specification. Later operations must validate
actual/declared lengths and arithmetic before allocating; construction alone
does not prove any particular object will fit. Host-sized maximum values are
representable policy values, not instructions to allocate that much memory.
