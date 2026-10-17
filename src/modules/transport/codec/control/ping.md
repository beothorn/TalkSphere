# PingToken

Represents eight opaque bytes used by PING and echoed unchanged by PONG. It has
no public fields. Connection logic, not the token, correlates replies and times
out outstanding requests.

- `new(bytes)`: retain exactly eight caller-supplied bytes; every value is valid.
- `decode(payload)`: require exactly eight bytes, otherwise return LengthMismatch;
  retain a copy without interpreting it or allocating a buffer.
- `encode()`: return the original eight bytes, suitable for either frame type.

Tokens can be copied and compared for equality but have no automatic diagnostic
representation. Trace events identify operations without recording token bytes.
