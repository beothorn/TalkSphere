# GoAway

Represents GOAWAY's payload shape independently of connection draining. It has
no public fields and borrows its diagnostic reason from caller-owned text or
payload bytes; those must remain available while the value is used.

- `new(last_accepted_remote_stream_id, code, reason)`: retain the full unsigned
  32-bit ID, any registered code including NoError, and valid UTF-8 text no longer
  than 1,024 bytes and without a leading BOM. Return a text validation error when
  these requirements fail; outgoing text must contain no secrets.
- `decode(payload)`: require at least eight bytes; read the big-endian four-byte
  ID, two-byte code and two-byte reason length, then exactly the declared UTF-8
  reason. Reject truncation, trailing bytes, unknown codes and invalid text.
- `encode()`: allocate and return the exact ID/code/length/reason bytes, bounded
  by 1,032 bytes. Do not normalize text.
- `last_accepted_remote_stream_id()`: return the ID unchanged, including zero.
- `code()`: return the registered shutdown code.
- `reason()`: borrow the untrusted diagnostic without interpreting or logging it.

The driver must still validate parity, the greatest actually opened ID and
monotonic limits across repeated GOAWAY frames. These checks need connection
state and cannot be established by a payload parser. Values can be copied and
compared; automatic diagnostic formatting is deliberately absent.
