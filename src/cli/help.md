# Shared help definitions

The topic table drives concise overview and detailed command/option help, including
option-spelling aliases. Every command and option has a discoverable topic; adding
one requires parser, README and CLI test updates together.

Overview reports the supported Linux authenticated Transport server and pending
application services. Serve help explains protected key loading, public readiness,
visible frame payloads, unsupported application protocols, limits, shutdown signals
and exit codes. Identity help covers explicit initialization/inspection and shared
store selection. Logging help documents RUST_LOG target/level filters on stderr.

No arguments show help successfully; invalid syntax exits 2 before effects. Missing
or inconsistent runtime credentials, unsupported platforms and runtime failures exit
1; normal shutdown exits 0. Help/version do not initialize operational logging or
start networking. The application version is not a conformance declaration.
