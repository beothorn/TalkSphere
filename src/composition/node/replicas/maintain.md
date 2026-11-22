# Replica maintenance

A background loop (started with local control, stopped before shutdown drains peer work) runs one maintenance pass a second.
The pass asks the pure planner what is due and performs it through the same peer operations an operator would run, so
every check and renewal is validated, journaled, charged once and bounded by `redundancy.maxSpend`, in the holder's credits.
The holder is reached over its live connection (a client that connects outward needs no address).

- **Plan** (`plan`): for each confirmed replica of each item, in order:
  1. retention ended (`expires_at <= now`): **expire**, the replica stops counting and the item needs another copy;
  2. else if an attempt failed less than `redundancy.retrySeconds` ago: wait;
  3. else if at most `redundancy.renewBeforeSeconds` remain: **renew** by the wanted retention;
  4. else if the policy's next check time has passed (one `checkSeconds` interval after the last conclusive check or the
     confirmation, plus a per-replica stagger of `staggerPercent`): **check** one sampled byte against the sealed bytes this
     node keeps.
  Every expiry is planned; at most four paid operations per pass, renewals (soonest expiry first) before checks (longest
  overdue first).
- **Outcomes**: a passed check records the check time; a conclusive failed check removes the replica from the count (the
  trust policy has already seen the failure); a renewal records the new expiry. An unreachable holder, a holder out of
  credits, a price above the bound or an inconclusive check is remembered and retried after the retry delay and never
  changes the replica count: it is proof of neither presence nor absence.
- **Repair**: a replica that expires or fails is dropped, so the item is offered again to the next client that fulfils
  the wanted offering (see [replicas](mod.md)); the node cannot push a copy to a client that has not offered.
- `Attempts` remembers incomplete attempts (bounded); `Maintenance` is the loop, stopped by dropping it.
