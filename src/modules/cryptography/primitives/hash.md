# Exact SHA-256

`sha256(bytes, maximum_bytes)` returns a 32-byte digest or `ResourceLimit` before
processing excessive input. No serialization or text normalization occurs.
`Sha256State.new(maximum_bytes)` starts an ordered incremental operation with that
local total-byte budget. `update(bytes)` appends exact bytes, including empty
chunks; a rejected oversized update changes neither state nor budget. `finish()`
consumes the state and returns its digest, preventing further updates/finalization.
Zero capacity permits hashing empty input. The established sha2 0.10 backend is
pinned in Cargo.lock; known-answer tests cover CR-01 and all abc partitions.
