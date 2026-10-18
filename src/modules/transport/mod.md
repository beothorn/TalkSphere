# Transport exports

Public domains provide local requests/events, scoped handles, configuration,
ports, codecs, frame I/O, handshake/integrity, streams, failure policy, shared
resources, connection composition, the control driver and TCP/clock adapters.

`start` is the normal CLI composition entry point. It currently returns
`TransportError.RuntimeUnavailable` because real Identity/Cryptography adapters
are absent. It performs no I/O. The library driver can be exercised with supplied
dependencies; deterministic substitutes exist only in test builds.
