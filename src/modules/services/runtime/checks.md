# Evidence-check coordination

admit_check requires coordinator authority, exact registered implementation,
explicit authenticated role consent, challenge-only input, full quote hash,
current Board terms and installed remote Rules. Local-only checks fail Unsupported.
It records accepted challenge/price/issuer/timing/allowance before a real positive
Ledger hold. Included checks reserve only their existing finite entitlement;
accepted attempts retain their usage guard even after interruption. Mutable
Board/issuer/version changes cannot reset a stable allowance.

supply_input independently validates check challenge rules. A local provider start
journals its attempt, calls only Provider.evidence under the accepted response
budget, validates complete evidence and retains it durably. It never receives the
expected byte. Re-reading evidence is not a recovery strategy after reply loss.
Paid checks prepare/commit the original Ledger charge; included checks perform
no Ledger reservation or financial effect. Complete valid wrong evidence may be
paid fail; malformed/incomplete evidence cannot authorize a charge.

outbound_check records one requester dispatch and returns the defined challenge.
check_sent records a trusted local complete-send clock stamp. receive_check accepts
only complete original-provider/stream evidence with local receive timing, rejects
known lateness and preserves the first retained response. check_timeout records
an observed deadline without complete valid evidence as an uncharged abort;
callers must order local receive/timer observations and cannot synthesize this from
an IPC disconnect. A different clock epoch remains unknown rather than proof of
elapsed time. Late data cannot resurrect an aborted check.

check_evidence returns complete retained evidence only after durable local success.
check_outcome runs only for the local requester with private expectations and
retained timing; no automatic refund or trust update follows. Missing evidence is
Inconclusive. Ordinary result/verify APIs reject check-purpose records so evidence
cannot be mistaken for a provider-calculated correctness outcome.
