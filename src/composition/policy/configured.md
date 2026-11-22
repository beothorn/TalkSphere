# Configured policy

`Configured` implements all six [contracts](contracts.md) from one validated [parameter set](params.md).
`Configured::policy(params)` returns the bundle; `Params::default()` reproduces the minimum node.

- **Admission**: refuse with `ConnectionLimit` when the node's live count is at `connections`, else with
  `PeerConnectionLimit` when the peer's count is at `perPeerConnections`.
- **Quota**: `pending`, `requestsPerMinute` and `failuresPerMinute` as configured.
- **Trust**: a passed check or verified contribution raises trust by its step, never above `ceiling` and never
  lowering a higher value; a failed check or failure lowers it by its step, not below zero. No change is `None`.
- **Credit**: one bootstrap grant per identity with no earlier bootstrap, while the total stays within
  `bootstrapBudget`; wanted terms as configured; release credits are `releasePercent` of cumulative payments,
  at least the sender's minimum and at most the payments (a minimum above the payments is returned unchanged
  so the caller refuses).
- **Redundancy**: target as configured; holders chosen among connected non-holders by fewest recorded failures,
  then highest trust, then input order; the next check is one interval after the last plus an offset of
  `staggerPercent` of the interval times the replica index modulo ten, divided by ten. Saturates at `u64::MAX`.
- **Connection**: ping interval and timeout as configured; reconnect delay doubles from one second to the cap.
