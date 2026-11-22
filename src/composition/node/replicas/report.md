# Redundancy report

`report(item, now, policy, check_seconds)` states what can be said about an item from observations only, following
[CR2SE Redundancy](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/Redundancy.md).

- **State**: from the number of confirmed, *unexpired* agreements. Fewer than two: `not redundant`. At least two but
  fewer than the local target: `under-replicated`. At least the target (and two): `target satisfied`.
- **Separately**: the local `target` (what is wanted), `confirmed` (what is observed), and `targetMet`. A target of one is met
  by a single copy that is still `not redundant`; an attempt or a requested target is never reported as redundancy.
- **Risk** (`atRisk`): a replica's retention ends within the renewal lead time (`expiringSoon`), a replica's last
  conclusive check (or its confirmation, if never checked) is older than two check intervals (`staleObservations`), or an
  agreement has expired and awaits dropping (`expired`).
- `target satisfied` describes observations, not a guarantee of future availability. Replica expiry times and last checks are
  listed so stale observations are visible.

`node-ctl replicas` prints this report for every item, followed by its replicas.
