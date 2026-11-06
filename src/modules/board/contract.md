# Direct-call context and snapshots

Context carries a borrowed local grant, monotonic deadline and cancellation
observation. check rejects cancelled/expired calls before validating the issuer,
publisher, action and byte budget. Adapters own concurrent cancellation delivery;
blocking commits already in progress are not falsely claimed rolled back.

Snapshot is owner-created and opaque. It exposes the publisher and borrowed exact
immutable Board bytes, and clones pin the complete associated revision. Provider
validates ownership for definition lookup; public bytes already returned are owned
results, not authority. Snapshot association alone is not authenticated remote
provenance; Cache has separate trusted receipt contracts.

Concrete Provider and Cache operations are the implemented ports. The initial
unavailable Boundary and declaration-only umbrella trait were removed after real
operations became available. Local contract version is 1, independent of Board
JSON schema version and peer protocol version. This direct compiled binding does
not claim a serialized version-negotiation protocol or expose local pointers/paths
as peer fields. See state.md, cache.md and selection.md for operation/error rules.
