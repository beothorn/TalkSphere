# Peer record framing

CONTROL_MAX is1048576 JSON bytes and DATA_MAX is65536 binary data bytes. Record
owns either exact Control JSON bytes or Body(id:uint32, offset:uint64, data).
Encode returns kind:uint8, length:uint32 big-endian and complete body, rejecting
empty/excessive payloads before output allocation. Binary body includes12 bytes
for ID and offset; data is nonempty. Framing alone does not validate JSON, body
association or message order.

Decoder is one default-empty stream direction. Push consumes at most one record
from a caller-owned slice and returns consumed count plus an optional complete
owned Record. Caller retains the unused suffix and repeats. Header storage is
fixed five bytes; kind and declared length are checked before body allocation.
No internal unbounded completed-record queue exists. Arbitrary DATA fragmentation
and coalescing preserve records. Malformed headers permanently poison the parser;
finish accepts only record boundaries and closes it. Partial terminal input is
Invalid, and all subsequent input after finish/error is Conflict. Exchange markers
and normal Transport END remain a separate state-machine responsibility.
