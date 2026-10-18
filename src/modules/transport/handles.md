# Local capabilities

ConnectionHandle controls one connection; its `id` exposes an observation-only
ConnectionId. ListenerHandle controls one listener. StreamOwner is issued only
by trusted protocol registration. StreamHandle binds a nonzero wire ID to a
connection instance and owner; `connection_id` and `wire_id` expose observations.
`authorize(connection_id, owner)` rejects foreign instances and owners before
any effect. Liveness is checked separately by the driver/session.

Constructors are internal, never derived from peer bytes. Clones deliberately
delegate the same authority. Equality compares process-local instance tokens;
no capability can be serialized, restored after restart or recreated from a
wire stream number. New connections cannot revive old stream handles.
