# Hosted publication byte-check judgement

The requester's conclusion about a paid byte check. The publisher keeps the content it signed, so it computes the byte the host must return; that `Expectation` never leaves the requester, never appears in a
challenge or log, and its `Debug` form omits the byte. `Verdict`: `Pass` (timely evidence for exactly this challenge matches), `Fail` (complete, timely evidence contradicts it) or `Inconclusive` (missing, late,
ambiguous or mismatched evidence, or no expectation; never a failure). `Timing`: `Within`, `Ambiguous` or `Missing`; `timing(elapsedMs, observed, responseSeconds)` classifies exactly (`Within` at exactly the
interval, `Missing` after it or when nothing arrived, `Ambiguous` only when a response was observed but could not be timed). `isValidEvidence` is a complete answer to exactly the challenge, which justifies the
charge whether or not the byte is right. `evaluate(challenge, evidence, expectation, timing)` returns the verdict; `Expectation::fromContent` derives the expectation from the content, or none for an offset outside it.
