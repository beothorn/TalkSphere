# Peer observations

An in-memory, bounded table of what this node observed about each authenticated identity.

- **Kinds**: `Connected`, `ContributionVerified`, `CheckPassed`, `CheckFailed`, `Refused`, `Failed`; plus
  `record_bootstrap(peer, credits, now)` for credits issued by bootstrap policy.
- **Summary**: `summary(peer)` returns first-seen time and saturating counters (unknown peers are all zero);
  `bootstrap_total()` sums bootstrap credits.
- **Bounds**: at most 4096 peers. When full, one entry is evicted before a new one is added: anonymous
  peers first, then peers with history, oldest first; a peer holding bootstrap credits is evicted only
  when nothing else remains, so a flood of new identities cannot erase granted peers first.
- **Ageing**: `age_out(now)` drops anonymous peers idle for 30 days and peers with history idle for 365
  days; a peer holding bootstrap credits is never dropped because forgetting a grant would allow another.
- **Trust**: only identities this node authenticated are keyed; no peer-supplied text is stored.
- **Document**: `to_json` / `from_json` use a strict `version: 1` shape with exact integers; unknown
  members, duplicates, bad identities and more than 4096 peers are refused.
