# Check

Requester-side judgement of a Messaging byte check. The sender keeps the payload it stored, so it can compute the byte a provider must return; that expectation stays private.

- `Verdict`: `Pass`, `Fail` or `Inconclusive`.
- `Timing`: `Within` (arrived inside the accepted response interval), `Ambiguous` (the time cannot be compared reliably) or `Missing` (nothing complete arrived, or too late).
- `Expectation::from_payload(placementId, payload, offset)`: the private expected byte, or nothing when the offset is outside the payload. Its debug form never shows the byte.
- `timing(elapsedMilliseconds, observed, responseSeconds)`: `Within` when the response took at most `responseSeconds`, `Missing` when later or absent, `Ambiguous` when a response was observed but could not be timed.
- `is_valid_evidence(challenge, evidence)`: true when the evidence answers exactly this challenge (same placement and offset). This, not correctness, is what justifies the check charge; a charge is never posted without it.
- `evaluate(challenge, evidence, expectation, timing)`: `Pass` or `Fail` only for `Within` timing, present matching evidence and an expectation for this very challenge, comparing the byte; everything else is `Inconclusive`, never a failure.
