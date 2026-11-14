# Challenge terminal gates

Receiver consumes Incoming under receiving Invoke authority and a trusted Clock.
marker accepts one original-stream input.end and captures the response deadline
then. Waiting for requester END cannot restart that interval. Duplicate/wrong
markers poison this gate. end requires both signals, independently validates the
challenge through Services and records one evidence dispatch using the already
captured deadline. Missing markers, expired timing and family/storage failures
cannot produce Dispatched. No detached challenge bodies or expectations enter
this supported bounded metadata path.

Dispatched retains the accepted check context and evidence binding for later
committed transmission. It does not assert successful commitment or remote receipt.

Sender consumes an already reserved Outgoing and queues accept then input.end
through bounded shared sequencing. outgoing borrows its original challenge-only
context. mark_sent requires all records queued and the local queue drained before
capturing one trusted send stamp and recording it with Services. Duplicate or
premature observations fail. Composition must order the actual input.end send and
END actions, and never synthesize timing from peer timestamps or IPC disconnects.

For asynchronous composition, marker_at accepts the original authenticated record
receipt Instant and performs the same authority/order checks before capturing its
deadline. A later worker invocation cannot restart the response window. This
instant is a trusted local observation, never a peer-supplied timestamp. The
legacy direct-call marker validates permission/order before consulting its Clock.
Expired original receipt windows cannot dispatch evidence, even when the worker's
current call deadline remains valid.

Sender.mark_sent_at records a trusted original local send/drain Stamp after the
same queue-complete and once-only checks as mark_sent. A worker can therefore
record the original observation after delay without sampling a new start time.
The stamp must use the same local epoch as the receive observation. Transport
queue admission is not remote delivery; composition preserves that distinction.
