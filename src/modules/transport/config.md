# Transport configuration

`ResourceLimits` contains maximum concurrent connections, active streams per
connection, total serialized outbound bytes and buffered incoming bytes per
stream. Limits are local policy, never wire negotiation. `TransportConfig.new`
takes these limits and a positive handshake timeout and rejects zero counts,
zero timeout, outbound budgets below 65,564 bytes and incoming budgets below
65,536 bytes. ConfigError preserves which limit failed and its minimum.
`limits` and `handshake_timeout` expose the validated immutable policy.
The driver enforces it before admission and through owned byte reservations.
