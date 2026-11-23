# Hosted Publications recovery

Before new work is admitted, `Participant::recover(decisions)` resolves every prepared operation from the Services coordinator's durable decisions: a recorded commit is applied
(idempotently, so an effect applied just before a crash is recognized), a recorded abort is released, and an undecided operation stays prepared with its locks. `DeferToServices` leaves
every operation undecided so the coordinator alone decides and delivers its recorded decisions with their retained times. New work is admitted only after every decision could be
consulted; a decision that cannot be read (`Unavailable`) or an effect that cannot be applied stops recovery with that error, new work stays refused and recovery can be run again.
It returns the counts committed, aborted and undecided; nothing is ever re-executed or retried on its own authority.
