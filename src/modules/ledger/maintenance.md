# Bounded Ledger maintenance

Maintain requires Administer and atomically compacts only zero balance entries.
It preserves trust, accepted terms, unresolved holds, terminal operation bindings,
all transfer replay records and all claim creation/terminal records. Usage reports
retained owned/issued accounts, operations, transfers and claims without secret data.

The conservative policy retains all operation history because no coordinator
retention proof is supplied yet. Admission refuses at capacity; it never resets
lifetime replay tables while keeping balances. Smaller reopen limits refuse the
existing image without rewriting or discarding obligations. Recovery validates
canonical schema, exact roles/resources, prices/effects, unique third-party payment
bindings and all active spending/incoming capacity before visibility. The private
schema version is explicit; unknown formats fail instead of implicit migration.
