# Recovery

`resume(coordinator, context, provider)` resumes every unfinished hosted publication operation from the coordinator's own durable record and returns how many reached a final decision. For each pending operation:
when the participant has no record of it, the coordinator aborts it (nothing was ever prepared); otherwise the coordinator re-delivers its recorded decision, with the retained decision time for stores.
Nothing is executed again and nothing is decided here. An operation that does not reach a final decision is left for the next run; running it twice changes nothing.

Startup order for a provider: recover the participant deferring to Services (`DeferToServices`), register the handlers, open the coordinator, call `resume`, and only then publish the offering.
Evidence: crash tests drive a store, a renewal, a removal and a get over three separately protected directories (the Services journal, the Ledger image and the hosted placements and participant journal),
lose or refuse the write at every coordination boundary (admission, staging, preparation, decision, commit acknowledgement, Ledger issuance), drop the whole node and reopen it from the files alone.
Placements, renewals and removals exist exactly when the matching charge does, each charge applies once, and early-release credits are issued once even when their acknowledgement was lost.
