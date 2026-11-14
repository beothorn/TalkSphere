# Locally authorized ordinary invocation

`Invocation` contains a sealed provided selection from the intended peer's direct
cache, that cache's current-contract view, complete locally chosen input, the
original maximum/payment tuple and a separately installed `Consent` policy. Peer
payloads cannot choose input, install policy or grant spending authority.

`Consent.approve` sees an independently validated immutable Candidate and returns
a locally approved integer-credit ceiling or a typed refusal. `Ceiling` is an
explicit local policy value; Client still validates exact original quote bytes,
hash, original peer/stream, installed schema/pricing, issuer/payment and both the
original and approved spending bounds. Merely constructing or signing an intent
is not the later consent/reservation step.

Acceptance revalidates current direct metadata and atomically reserves the actual
requester owner before emitting accept. The original input is sequenced as accept,
ordered bodies, input.end and local END, without advancing sources on backpressure.
Receiving early/unordered output is a stream error. Complete output remains
provisional until result.end and actual normal stream closure.

Only then does the original result receiver independently validate and stage the
result, followed by acknowledged prepare/commit of local participants. A Work
reply identifies that original committed key; it does not assert third-party
payment settlement. Normal explicit accepted failures use the original owner
abort path; interrupted results preserve holds and uncertainty. No automatic
retry, input coalescing or fresh financial attempt is introduced.

The explicit Ceiling value also implements the separate CheckConsent contract;
check schema, entitlement and timing validation remain independent of ordinary
work, as described in the [check request contract](check.md).
