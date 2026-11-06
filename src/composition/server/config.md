# Server configuration

Options is trusted local input: explicit identity-store path, listening IP/port,
copied contacts, contact capacity, validated Transport policy and shutdown timeout.
local(path) supplies loopback port zero, no contacts, capacity 64, 64 simultaneous
connections, 128 streams each, 64 maximum frames of outbound budget, one maximum
payload buffered per stream, a ten-second handshake and five-second shutdown.
These are application defaults, not protocol limits. Non-loopback binds require
an explicit address. Port zero is valid for binding, never for an outgoing contact.

Config.new rejects an empty identity path, zero shutdown deadline, zero/exceeded
contact capacity, duplicate binary contact IDs, unresolved or invalid endpoints, and monotonic deadlines
that the platform cannot represent. DeadlineRange is rejected before acquisition.
No storage inspection, creation or networking happens during construction. Existing
TransportConfig owns transport validation. options returns an immutable view; later
changes require a newly validated configuration. Config errors contain categories
only, never private paths or peer-supplied diagnostics. Stored-contact snapshot
selection is added by the CLI/startup steps before runtime composition, with no
implicit Discovery lookup or unbounded reconnect behavior.
