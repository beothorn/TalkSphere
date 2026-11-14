# Reactor scheduling and output ownership

Call `Bridge.poll` on every reactor turn, after Transport has had an opportunity
to progress, including turns with no application event. It registers the reactor
waker before examining completions. Up to64 completions are consumed per call;
more available work requests another turn. Results correlate both the worker
ticket and the stream generation, preventing a late result from selecting a
replacement stream.

Each stream owns at most one in-flight callback. Pending bytes stay charged until
that call returns. An output-blocked stream retains its exact framed bytes and
does not start further input callbacks. Each stream admits at most one65536-byte
DATA fragment per turn. Any admitted DATA or END requests another reactor turn
so Transport can actually write it, even without another application event.
Accepted local prefixes advance once; local Backpressure
advances nothing. Polling after subsequent Transport progress retries queue
admission without waiting for a nonexistent application-writable event. It never
repeats a service, transfer or other dependency operation.

END is submitted only after all pending output is admitted. Its local queue
acceptance is not remote delivery. A drained producer receives a worker-side
writable callback when more output can be generated. A stream deadline requests
cancellation while retaining any running operation and its eventual completion.

Malformed framing/order errors request owner-scoped PROTOCOL_ERROR; quota errors
request resource-limit cancellation. Other dependency failures request internal
cancellation. Termination intent survives local Transport backpressure. Peer
stream failure never closes an unrelated stream or fabricates an uncharged
application failure after possible effects.

Worker panic or invalid correlation is uncertain. A lost transient handler is
not restarted; composition must inspect the original durable owners. Callback
closures remain bounded by outstanding worker capacity. Normal cleanup calls the
handler and releases its state on a worker thread. No global logging subscriber,
raw payload, claim bytes, quote JSON or secret diagnostics are introduced here.
