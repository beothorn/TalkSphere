# PayloadError

Reports malformed control payloads without retaining or echoing diagnostic text.
It is a local validation result, not a wire error code. The future driver must
handle malformed payloads using the specification's connection-error rules.

| Variant | Public data and meaning |
|---|---|
| TooShort | `minimum` required bytes and `actual` bytes received |
| LengthMismatch | `expected` exact bytes and `actual` bytes received |
| ReasonTooLong | `actual` diagnostic byte count exceeds 1,024 |
| InvalidUtf8 | Reason bytes are not valid UTF-8 |
| ByteOrderMark | Reason begins with a prohibited BOM |
| UnknownCode | Unregistered unsigned 16-bit value |
| NoErrorForbidden | ERROR/CANCEL attempted to use GOAWAY's NoError |

Lengths refer to the complete payload, including GOAWAY's initial four-byte ID.
Formatting produces a readable explanation containing only safe numeric context.
The standard error interface reports no nested cause. Errors can be copied and
compared without interpreting their formatted message as machine state.
