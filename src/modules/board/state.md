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

Provider.new uses volatile Memory and makes no restart claim. Durable owners
use Provider.open with a declared persistence adapter and explicit recovery.

Durable owners use Provider.open with an explicit Persistence port. Successful
publication acknowledges persistence before atomic visibility. Unknown commit
outcomes block current reads/new publication until recover resolves stored state;
old immutable pins remain owned data. Provider.new explicitly uses volatile Memory.

A replacement may release the owner's unpinned current revision when calculating
retention admission; with snapshots=1, replacement works after external pins drop.
Capabilities are rechecked inside the writer serialization boundary before commit.
Composition must coordinate handler withdrawal with this owner rather than change
capabilities concurrently after a successful check and claim existing work vanished.
Public bytes already returned remain owned data after revocation; revocation blocks
future privileged operations and does not erase callers' copies.
