# Atomic local owner

Provider.new binds one publisher to trusted Authority, Limits and Capabilities.
It creates no publication until explicit publish. publish validates caller scope,
revalidates the complete candidate under the receiving owner's limits and current
capabilities, serializes writers, rechecks call control and installs one complete
revision. Different owners cannot use each other's pins. Concurrent writers use
last successfully serialized replacement; no field-level merging occurs.

read pins one immutable current snapshot or returns Missing. definition reads
only the exact ID from that pin and fails for missing/foreign data. Each operation
enforces its relevant byte budget. A pin retains the complete Board/definitions;
replacement cannot mutate old terms. Limits.snapshots bounds live revisions;
excess publication is refused until older pins release. A poisoned owner reports
Unknown instead of proceeding with possibly inconsistent state.

This initial owner is volatile. Durable storage is added through a separate port
in the next step; no restart guarantee follows from this in-memory adapter.
