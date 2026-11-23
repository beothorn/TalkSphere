# Page provider adapter

`PageProvider::new(identity, pages, journal, entropy, budget, maximumBytes)` serves the publisher's own identity.

- `assess`: only the publisher's own provided offering, only the authenticated remote's own operation (`Denied` otherwise); the request must fit the offering (canonical path within `maximumPathBytes`, a nonzero
  `maximumBytes` within `maximumResourceBytes`, else `Invalid` before any resource is read) and the page root must be usable (else `Unavailable`). The metadata is the complete request and there are no factors.
- `execute`: validates against the accepted offering, takes one snapshot, and stages the complete result with the fixed price (a missing resource is a concealed `Denied`, a resource above either bound is `Limit`; a
  failure leaves nothing prepared). The result is pinned in memory and a durable record of the request and the terms it was staged under is journaled as prepared. A repeated call returns the same staged result.
- `completion`: the pinned result, or after a restart a result rebuilt from the journaled request and terms (a changed resource then fails the coordinator's comparison rather than being substituted); `Unknown` when
  nothing was staged. `prepare` confirms the effect bytes and returns the token; `commit` records the outcome so a lost reply is answered from it; `abort` releases it; `status` reads the journal.
- `evidence`: `Unsupported`; the Page check is the visitor's own.
