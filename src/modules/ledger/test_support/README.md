# Ledger deterministic dependencies

This cfg(test)-only register distinguishes precommit failure from lost durable
acknowledgement. Its reopen method transfers only durable bytes. Script exhaustion
is an assertion failure, never implicit success. It does not implement or claim a
real filesystem transaction. Adapt it to the persistence port when that port exists.

Use public Identity/Transport vector credentials for real crypto adapter tests;
opaque byte identities in authority unit tests assert exact equality only and do
not claim authentication. Expected numeric acceptance values are literal in the
requirements map. Barriers control future races; randomness/receipt fixtures will
be added beside the contracts that consume them, without production fallbacks.
Run `cargo test --locked modules::ledger` from the repository root.
