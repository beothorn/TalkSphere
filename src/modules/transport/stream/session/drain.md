# Session draining

`drain(reason)` queues GOAWAY once and forbids new local OPEN. The bound is the
greatest remote stream actually accepted, excluding malformed/unsupported
requests. Reasons map to NO_ERROR, ID_EXHAUSTED, RESOURCE_LIMIT or INTERNAL_ERROR.
Repeated local drain is idempotent; closed sessions reject it.

Verified peer GOAWAY must have zero or local parity, cannot exceed the greatest
local OPEN and cannot increase a previously received bound. Higher active local
streams fail as refused; lower streams may finish. The session replies with
its own GOAWAY if needed. Correct next remote OPENs after drain consume their
IDs and receive REFUSED_STREAM. Local ID exhaustion initiates ID_EXHAUSTED drain.

`ready_to_close` becomes true only after local GOAWAY is fully written, no active
accepted streams remain and queued output is empty. The owning connection then
closes normally, preserving completed stream outcomes. It must also ensure no
frame remains partially written outside the session queue.

A nonzero peer GOAWAY reason is retained in the final typed connection outcome;
accepted streams can still complete normally. EOF after completed graceful
output and all accepted streams is normal drain, rather than connection loss.
EOF while accepted work remains unfinished still fails that work.
