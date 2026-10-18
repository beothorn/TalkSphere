# OPEN selection contract

`Open` borrows a validated protocol identifier and stores a nonzero 32-bit
version. Construction takes an identifier and version; decoding takes a complete
payload. Neither operation establishes that a consumer supports the selection.
The identifier is 1–255 ASCII bytes, starts and ends in a lowercase letter or
digit, contains only lowercase letters, digits, dots and hyphens, and has no
adjacent dots. Encoding writes its two-byte big-endian length, identifier bytes
and four-byte big-endian version. No trailing bytes are permitted.

`id` and `version` return these validated values. `encode` returns owned bytes;
`decode` retains a reference into its input, which must outlive the result.
`OpenError` distinguishes invalid lengths, invalid identifier grammar and zero
version. Routing maps all malformed selections to a stream protocol error after
consuming the correct next stream ID; a valid unknown selection is unsupported.

Behavior follows [CR2SE section 7](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md#7-opening-and-identifying-a-stream).
No socket, registry or cryptographic operation occurs here.
