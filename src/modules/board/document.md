# Lossless document

Document.parse accepts one complete UTF-8 JSON value and validated Limits.
It rejects duplicate decoded member names at every depth, invalid escapes,
trailing data, malformed syntax and excess bytes/depth/nodes/array/string sizes.
Preflight checks run before allocating the full logical tree. Total input bytes
also bound temporary string decoding; decoded strings have their own budget.
The immutable tree uses arbitrary-precision JSON numbers. bytes returns exact
original input, including unknown nested metadata and number spelling. value
borrows the logical tree. Clones share immutable source bytes; neither operation
performs I/O or trusts document contents as authority.
