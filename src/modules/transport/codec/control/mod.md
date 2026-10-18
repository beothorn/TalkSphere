# Control codec module

The Transport connection state use this module to convert complete
control payloads between bytes and validated values. It re-exports PingToken,
ErrorPayload, GoAway, ErrorCode and PayloadError, whose contracts are linked in
the [module README](README.md). MAX_REASON_BYTES is the fixed value 1,024 and
counts UTF-8 bytes. There are no module-level functions or mutable public fields.

Payload parsing does not authenticate a peer or enact connection/stream state
changes. The enclosing header supplies frame type and stream scope. The module
performs no I/O; logging is routed through the application's tracing subscriber.
