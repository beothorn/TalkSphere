# Authenticated stream session

`Session.new` takes TCP role and authenticated local/peer identities from
successful handshake composition. It issues connection authority, starts stream
numbering and queues an authenticated event. `handle` exposes that authority to
trusted composition. This constructor does not itself verify identity claims.

`open(registry, owner)` requires a registered owner, consumes the next local ID,
installs scoped stream state, queues a stream-opened event and queues OPEN bytes.
It returns a stream capability. Invalid ownership and exhaustion are local
errors. `receive(verified_frame, registry)` accepts only integrity-verified input.
Remote OPEN consumes exactly the next ID before payload validation and lookup.
Malformed selections queue PROTOCOL_ERROR; valid unsupported selections queue
UNSUPPORTED_PROTOCOL. Neither refusal creates a consumer stream or invalidates
other streams. Reuse or gaps return a connection-level ID error.

`pop_event` transfers the oldest event for composition to route; no consumer
callback runs under internal state mutation. `pop_outbound` transfers an unsealed
frame to the serialized integrity/writer path. Both return no value when empty.
`SessionError` distinguishes ID, ownership, protocol and internal framing errors.
The connection driver must terminate on connection errors. DATA and lifecycle
routing are added in subsequent steps; unsupported frames currently fail
explicitly. Queue enforcement is planned in steps 20–21.
