# Stream ID history

`StreamIds` belongs to exactly one connection. Construction takes its TCP role;
initiators allocate odd IDs starting at 1, acceptors even IDs starting at 2.
`allocate` consumes the next local ID, then advances by two without wrapping.
`accept(id)` requires exactly the next remote ID and consumes it before OPEN
payload validation, support checks or resource refusal. Rejected IDs remain used.
`IdError` distinguishes local exhaustion from an unexpected remote ID.

`was_created(id)` consults constant-space high-water marks, with zero always
false. An absent active entry for a previously created ID is terminal. This
avoids unbounded tombstones. `is_local(id)` tests creator parity, while
`greatest_local` and `greatest_remote` return the watermarks, initially zero.
Callers serialize allocation and admission; no method performs I/O or creates
capabilities. Handles remain scoped to connection instances separately.
