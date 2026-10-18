# Connection failure policy

`Disposition` contains a typed consumer failure and optional wire error code.
`framing(error)` maps oversized payloads to FRAME_SIZE_ERROR, other trustworthy
header violations to PROTOCOL_ERROR, truncation to a distinct local outcome,
and socket loss to connection loss. Unsupported versions, I/O failures and
ambiguous phases permit no response. No path resynchronizes an invalid frame.

`handshake(error)` maps malformed order/payload to PROTOCOL_ERROR and failed
identity/cryptographic verification to AUTHENTICATION_FAILED. A peer ERROR
preserves its code and peer origin and permits no response. `integrity(error)`
always permits no response and distinguishes sequence exhaustion from failed
integrity. `with_safe_output(false)` suppresses any otherwise permitted response.

The driver must close the socket, fail unfinished streams and release keys.
A permitted response is best-effort, with known direction phase and intact
output framing; it must never be appended halfway through a failed write.
