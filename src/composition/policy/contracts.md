# Policy contracts

Every contract is a pure function of a **peer view**: the peer identity, its trust in millionths
(`0..=1000000`), a bounded observation **summary** and the current Unix time.

## Data

- **Summary**: first seen, connection count, verified contributions, checks passed and failed,
  refusals, failures and bootstrap credits issued. All counters are bounded.
- **Verdict**: `Allow`, or `Refuse(reason)` with one of `ConnectionLimit`, `PeerConnectionLimit`,
  `RateLimited`, `BudgetExhausted`, `Disabled`. A refusal happens before any work, charge or reservation.
- **Limits**: simultaneous pending operations, requests per minute, failed attempts per minute.
- **Event**: `CheckPassed`, `CheckFailed`, `ContributionVerified`, `Failed`. Only verified observations.
- **Grant**: finite credits plus the trust assigned with them. **Wanted**: credits per object,
  largest object and retention of the wanted Storage offering.
- **Candidate**: a possible replica holder with connection state, history and trust.

## Contracts

| Contract | Question | Answer |
| --- | --- | --- |
| Admission | May this authenticated connection stay open, given node and per-peer connection counts? | Verdict |
| Quota | What finite limits apply to this peer? | Limits |
| Trust | Does this verified event change the peer's trust? | New trust, or none |
| Credit | Which first grant, wanted terms and release amount? | Grant, Wanted, credits |
| Redundancy | How many independent copies, which holders, when to check next? | Target, holders, time |
| Connection | Ping interval and timeout, reconnect delay for attempt n | Seconds |

A **Policy** bundles one implementation of each. Implementations must be deterministic for equal
inputs, must not block or perform I/O, and must bound every output.
