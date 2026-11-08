# Services module boundary

Services owns schema/value validation, authorized handler registration and local
invocation coordination. Board consumes schema formats without invoking runtime.
Composition supplies authority and dependencies; Peer Protocol owns network I/O.

Error distinguishes invalid input, unsupported capability, denied authority,
resource limit, cancellation, conflicting reuse, insufficient capacity, known
precommit storage failure, possibly committed unknown outcome and unavailable
dependency. Unknown never establishes rollback. Authority and contract submodules
specify local version, identity, action, deadline and resource checks.

The runtime coordinator remains under development; these types alone do not
register handlers or execute work.
