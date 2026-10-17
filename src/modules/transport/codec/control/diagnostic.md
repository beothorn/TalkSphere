# ErrorPayload

Represents the identical ERROR and CANCEL payload format, independent of frame
type or stream scope. It has no public fields; its borrowed reason remains valid
only as long as the caller's input text/buffer remains available.

- `new(code, reason)`: accept a registered nonzero code and UTF-8 text of at most
  1,024 bytes without a leading BOM; reject NoError, excessive length or a BOM.
  The caller must ensure the text contains no secrets.
- `decode(payload)`: require a two-byte big-endian code, two-byte big-endian
  reason byte length and exactly that many reason bytes. Reject short/trailing
  input, oversized reasons, invalid UTF-8, a leading BOM, unknown codes and
  NoError. Borrow the validated reason without copying it.
- `encode()`: allocate and return the exact code, length and text bytes, bounded
  by 1,028 bytes. Preserve UTF-8 bytes without normalization.
- `code()`: return the registered code for machine-readable decisions.
- `reason()`: borrow the untrusted text for optional diagnostics, without logging,
  escaping or interpreting it. Consumers must handle display/logging safely.

Payload values can be copied and compared, but no automatic diagnostic formatter
exposes their reason. Trace events contain operation context only. Successful
parsing does not authenticate bytes, choose error scope or terminate a stream.
