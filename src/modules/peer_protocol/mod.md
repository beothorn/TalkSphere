# Peer Protocol boundary

Peer Control owns framing, exchange/quote ordering and detached body transfer.
Board owns metadata, Services owns accepted work and Ledger owns accounting.
Error distinguishes invalid, unsupported, denied, limit, cancelled, conflict,
insufficient, known storage refusal, unknown effects and unavailable dependency.
Only the scoped local contracts are implemented so far; production dispatch is
not registered and no wire support is advertised.

Services error conversion preserves every typed category one-for-one, especially Unknown versus known refusal; wire failure-code mapping belongs to the exchange handler.
