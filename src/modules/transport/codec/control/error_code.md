# ErrorCode

Defines version 1's unsigned 16-bit codes. Numeric conversion to a code rejects
every unregistered value with UnknownCode; conversion of a code to its wire
number preserves the table below. No text lookup or application extensions exist.

| Wire value | Name | Meaning |
|---|---|---|
| 0x0000 | NoError | Graceful GOAWAY only |
| 0x0001 | ProtocolError | Malformed or forbidden behavior |
| 0x0002 | AuthenticationFailed | Identity or handshake failure |
| 0x0003 | FrameSizeError | Payload above the frame limit |
| 0x0004 | UnsupportedProtocol | Unsupported OPEN protocol/version |
| 0x0005 | StreamStateError | Invalid existing-stream transition |
| 0x0006 | RefusedStream | Stream not accepted |
| 0x0007 | InternalError | Implementation failure |
| 0x0008 | IdExhausted | No local stream IDs remain |
| 0x0009 | Cancelled | Intentional cancellation |
| 0x000a | ResourceLimit | Local bounded resource exhausted |

`try_from(value)` accepts a numeric wire value and returns its code or
UnknownCode. This lookup does not decide which frame permits a code or whether
the failure affects a stream or connection. Codes are copyable and comparable.
