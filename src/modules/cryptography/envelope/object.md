# Complete encrypted objects

`object_length(header, limits)` validates sealed length at least 16, unsigned
addition of the 132-byte header, conversion to host size and both local byte
budgets. Stream callers use it before reading or allocating the body.
`lengths_for_plaintext(plaintext_length, limits)` returns (sealed bytes, total
bytes) after checked tag/header addition and the same limits. No allocation occurs.

`parse(bytes, limits)` requires one complete object with no missing or trailing
bytes, then returns a ParsedObject: decoded `header`, borrowed exact
`encoded_header`, and borrowed `sealed` ciphertext/tag. Views live no longer than
the supplied bytes and imply structural validity only, not authentication.
Invalid format/truncation yields `InvalidInput`; overflow, host conversion and
budget excess yield `ResourceLimit`. No provisional plaintext can be returned.
