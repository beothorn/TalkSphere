# Messaging recovery

`Participant::recover(decisions)` runs at startup, after the Services coordinator has recovered its own decisions and before any new work is admitted. For each prepared operation in the journal it asks the coordinator's durable decision (`Decisions::decision`):
`Commit` applies the effect (idempotently, so an effect applied just before a crash is recognized and keeps its original acceptance time), `Abort` releases it, `Undecided` leaves it prepared with its placement locked. Only after every decision could be read does it admit new work; a decision that
cannot be read stops recovery with `Unavailable` and new work stays refused, and recovery can simply be run again. It never re-executes or retries anything on its own authority. The `Report` counts committed, aborted and undecided operations.

Evidence: a fresh participant refuses work until recovery has run; after a restart each prepared store is committed, aborted or left undecided by the table of decisions, with the undecided placement ID still locked; the window after applying and before journaling is repaired with the original time; an unreadable decision keeps work refused.
