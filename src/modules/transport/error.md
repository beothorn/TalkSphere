# Immediate local Transport failures

TransportError distinguishes unavailable runtime composition, invalid config,
wrong connection/owner authority, invalid endpoint/request, OS I/O error kind,
startup failure, dependency failure, backpressure and closed resources.
Config and startup failures retain their typed causes. Display text is safe
local explanation, not peer-provided diagnostic text. Backpressure means the
request was not admitted and may be retried after capacity becomes available.
