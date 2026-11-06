# Transport exports

Public domains provide local requests/events, scoped handles, configuration,
ports, codecs, frame I/O, handshake/integrity, streams, failure policy, shared
resources, connection composition, the control driver and TCP/clock adapters.

`start` is an argument-free compatibility helper that returns RuntimeUnavailable
without I/O because it cannot choose protected credentials or policy. The configured
CLI now uses composition::server with real Identity/Cryptography, TCP and timers.
The library driver remains usable with explicitly supplied dependencies; deterministic
substitutes exist only in tests. See the server integration acceptance matrix.
