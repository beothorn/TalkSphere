# Recovery

`resume(coordinator, context, provider)` resumes every unfinished Messaging operation from the coordinator's own durable record and returns how many reached a final decision. For each pending operation: when the participant has no record of it, the coordinator aborts it (nothing was ever prepared); otherwise the coordinator re-delivers its recorded decision, with the retained decision time for stores and recoveries. Nothing is executed again and nothing is decided here. An operation that does not reach a final decision is `Unknown` and recovery can be run again; running it twice changes nothing.

Startup order for a provider: recover the participant deferring to Services (`DeferToServices`), register the handlers, open the coordinator, call `resume`, and only then publish the offering.
