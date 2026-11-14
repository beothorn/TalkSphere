# Peer Protocol local contracts

Local version1 is independent of cr2se.peer wire version1. OperationKey contains
the authenticated requester identity, trusted unique32-byte connection instance
and uint32 stream instance; it is correlation, never authority. Transport stream
handles must be associated by trusted composition, not parsed from payloads.
Opener identity does not determine application roles.

Context borrows a receiving-owner Grant and supplies version, absolute monotonic
deadline and cancellation. Check rejects incompatible versions, expired/cancelled
calls, foreign/revoked domains, wrong identity/action and excess byte budgets.
An expired call never proves remote rollback.

Limits defaults: handlers4096, concurrent exchanges4096, live bodies65536,
logical nodes65536, unaccepted quotes64 and state/queued bytes16777216. Positive
limits and checked framing headroom are mandatory. These are local admission
limits; they cannot reduce the required1048576-byte control and65536-byte binary
data record support. Exchange engine admission must charge aggregate resources;
these contract declarations alone do not enforce queues or rate policy.

Additional admission defaults are256 connections,16777216 queued bytes and4096 control messages per composition-configured monotonic rate window. These are independent positive budgets; enforcement and rate-window ownership belong to the exchange engine.

Association is an observation containing local identity, authenticated peer
identity and the complete OperationKey. Trusted Transport composition supplies
it and the incoming-event grant; it is not decoded from peer JSON or itself a
financial capability. Metadata requests correlate all fields before attribution.

Value parsing applies `bodies`, `nodes` and `state_bytes` to each logical value;
received nonempty detached descriptors may be just one byte. The configured
Endpoint additionally reserves shared logical-byte/body capacity across handlers
and peers through [aggregate admission](engine/endpoint/resources.md). Transport
framing, queued input/output and worker admission have their own explicit Bridge
configuration. Operators must compose these independent budgets together.
