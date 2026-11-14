# Provisional result reception

ResultReceiver.new binds a real reserved Outgoing requester attempt, incoming
Invoke authority, original Association, immutable Invocation, finite limits and
local deadline. Composition must enforce Exchange ordering, including requester
END before ordinary result handling, and route only authenticated stream events.

control permits one result metadata control and result.end only after exact
bodies. Fixed price must match immediately; metered price cannot exceed the
accepted maximum and remains subject to complete-record formula validation.
body accepts only ordered declared bytes between those controls. Wrong attribution
cannot mutate another stream; malformed/early/late data poisons this receiver.
No provisional output or plaintext is exposed through an application accessor.

finish requires result.end, exact bodies and trusted normal completion of the
original exchange. Abnormal completion remains Unknown. It validates output
price fields, stores the complete logical output with the local Services codec
and submits RemoteResult to the original Coordinator. That codec is an opaque
local durable representation, not the Peer Protocol wire encoding or a claim of
interoperability. Services independently checks schema, family content, quoted
sizes and final computation arithmetic before staging the result. Applications
obtain success only through the coordinator's subsequent committed result port.

Missing/extra bodies, invalid text, missing markers, changed original content,
price errors, cancellation or storage uncertainty cannot publish success. Accepted
holds remain available for original-key recovery; interrupted output is never an
implicit uncharged failure or authorization to repeat work.

A failure control before result.end discards provisional output and retains a
bounded diagnostic. failure exposes that diagnostic only; it is not a settlement
decision. finish applies it through the original coordinator only after normal
completion, acknowledging abort before releasing uncommitted capacity. Abnormal
completion preserves Unknown even if a failure control was received. A failure
after result.end poisons the exchange and cannot overwrite committed work.

for_retry consumes an explicit Services Retry and binds its approved new stream to
the original accepted invocation. Complete normal results use receive_replay inside
the original coordinator; no adapter forges old Transport provenance. Failure or
interruption on a retry never releases or contradicts the original obligation.
