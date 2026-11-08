# Services module boundary

Services owns schema/value validation, authorized handler registration and local
invocation coordination. Board consumes schema formats without invoking runtime.
Composition supplies authority and dependencies; Peer Protocol owns network I/O.

Error distinguishes invalid input, unsupported capability, denied authority,
resource limit, cancellation, conflicting reuse, insufficient capacity, known
precommit storage failure, possibly committed unknown outcome and unavailable
dependency. Unknown never establishes rollback. Authority and contract submodules
specify local version, identity, action, deadline and resource checks.

The [runtime coordinator](runtime/mod.md) implements durable local invocation and
evidence-check coordination through explicit installed handler/accounting ports.
The [local adapter](adapter.md) preserves these contracts under bounded serialized
calls. Neither module installs a standard family or network application protocol.
