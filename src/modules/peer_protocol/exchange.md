# Directional exchange ordering

Direction is Local or Remote relative to trusted composition. Exchange.new takes
the Transport stream opener, never a payload identity. Roles initially assign
requester=opener, provider=other; offer reverses them for wanted work. Kind is a
structurally validated message discriminator. Control advances only specified
metadata, invocation/check, wanted and Ledger phases and sender directions.
Body permits only accepted requester input or provider output. The owning Receiver
must separately prove exact body contents and complete schema/price validity.

Invocation input.end precedes requester END, then result/output bodies,
result.end and provider END. Metadata/simple Ledger requests may half-close after
the request; transfer owners must wait for fee acceptance (or a stored direct
result) before END. Failure is permitted before terminal success from the proper
responder, including wanted publisher refusal; failure after result.end is illegal.
Unrelated early results never use a cached-family exception. That exception needs
an independently authorized family port in the later retry step.

End validates each half-close once. Close consumes final normal/abnormal Transport
observation: Complete requires both legal ENDs and terminal reply; explicit Failure
requires complete failure ordering and normal closure. Interrupted accepted work
is Unknown, never evidence of rollback. Illegal transitions poison only this
instance and preserve its accepted guard. Accepted reports possible effects even
when a later protocol error prevents normal completion. A pre-acceptance Failure
classification does not erase prior independent Ledger/family replay obligations;
callers must consult authoritative owner state. Local grants, semantic fields,
exact quote consent, body completeness and participant commitment are separate
mandatory checks before any effect. This pure ordering model performs no I/O,
Ledger mutation, code installation, work retry or application dispatch itself.

cached_local accepts original-owner Cached proof and cached_remote accepts an
explicit requester Retry. These are the only early-result exception: matching
propose must already be observed, the result sender must be provider, and no new
quote/accept/input body is introduced. The transition itself observes result;
callers must not apply an additional ordinary result transition. Requester END is
then permitted during cached output. Missing bodies/markers or interruption still
cannot become success. Composition must correlate proof to the actual authenticated
association before invoking this ordering-only helper.
