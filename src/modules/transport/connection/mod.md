# Owned connection composition

`Options` selects TCP role and expected-peer pin. `Connection.begin` takes an
owned socket, options, validated configuration, monotonic time and Identity/
Cryptography capabilities. It requests fresh handshake material and queues
HELLO. Startup failures close the socket and return a failure disposition.

`handle` retains the same process-local authority across authentication.
`session` exposes stream operations only after verified remote AUTH and fully
written local AUTH. `pop_event` transfers queued observations without callbacks.
`poll(context, now, registry, identity, crypto)` performs bounded nonblocking
work: serialized writes, complete-frame reads, directional handshake transition,
integrity verification and stream dispatch. It returns ready only after socket
closure. Consumers must not treat Pending as failure.

`next_deadline` returns the earliest handshake/error-output/probe deadline.
The parent registers this with its injected Clock and calls poll when due.
Handshake deadlines cover stalled reads and writes. Error replies are bounded
by the same timeout policy. Unsupported versions and failed tags close silently;
other errors get a best-effort response only with trustworthy idle output state.

`close` immediately closes and fails unfinished work. Drop also releases the
socket, framing buffers, ephemeral credentials and directional keys. Completed
stream outcomes are not converted to failures; EOF never completes unfinished
streams. Final bounded outcome events remain drainable after resources close.

`begin_shared` accepts an additional shared Resources instance. It reserves a
connection slot and HELLO bytes before requesting credentials. Saturation closes
the supplied socket and returns RESOURCE_LIMIT without authentication calls.
Normal `begin` creates standalone budgets; a multi-connection driver must use
shared budgets. Authenticated sessions inherit their parent's resource policy.
