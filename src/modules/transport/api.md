# Transport request and event contract

Trusted composition submits `Request` values through `TransportControl.submit`
and polls queued events through `poll_event`; pending polls arrange a wakeup.
`Accepted` reports admission, never remote delivery. Immediate rejections
return TransportError without queueing work.

Connection roles identify TCP initiator/acceptor. Expected peers are either a
known identity pin or explicitly unpinned (still requiring authentication).
`ConnectRequest.new(endpoint, expected_peer)` rejects outgoing port zero;
endpoint and expected_peer accessors retain those inputs. Listen permits port
zero. Accept transfers socket ownership. StopListening and Close require the
corresponding local capability.

`Protocol` contains identifier text and a 32-bit version; the OPEN codec validates
it. RegisterProtocol returns owner authority. OpenStream takes connection and
owner capabilities. Stream requests combine owner, stream and Data, End or
Cancel action. Data is ordered bytes; empty sends are invalid. Cancellation
reasons are intentional cancellation, resource limit or internal error.
Ping takes a unique outstanding eight-byte token and positive timeout. Drain
carries a normal, exhaustion, resource or internal shutdown reason.

Admission returns a pending connection handle, listener handle/address,
registration owner, stream handle or Queued. `PeerContext` carries connection
observation identity, TCP role and authenticated local/peer identities.
Events report incoming connections, authentication, opened streams with context
and protocol, verified DATA, remote END, terminal stream outcome, ping result,
connection closure and listener closure. Composition routes events to consumers
without callbacks during internal mutation. Ping results contain elapsed time
or a failure; normal connection closure has no failure.

Failures preserve Local, Peer or named Dependency origin and a machine-readable
code. Codes distinguish protocol, authentication, integrity, frame size,
unsupported protocol, stream state, refusal, cancellation, resource limits,
ID/sequence exhaustion, truncation, connection loss, deadlines, unavailable
dependencies and internal errors. `Cancelled` preserves the explicit peer
CANCELLED code rather than treating it as refusal. Stream outcomes distinguish
normal completion after both ENDs, cancellation with origin/reason and failure.
No outcome causes automatic retry or application rollback.
