# Observation store

`FileObservations` adds protected atomic persistence to the [peer observation table](../policy/observations.md)
in `observations.json` inside the node root.

- `open(path)`: an absent file starts empty; an oversized (over 2 MiB), unreadable or malformed file is
  `Corrupt` and is never replaced.
- `record(peer, kind, now)`, `age_out(now)`: change memory and mark the table dirty.
- `record_bootstrap(peer, credits, now)`: written before it returns, so a crash cannot allow a second grant.
- `flush()`: writes only when dirty, via a private temporary file and rename with owner-only permissions; a
  write failure leaves the table dirty so a later flush retries.
- `summary(peer)`, `bootstrap_total()`: read without marking dirty.

Observations are advisory: a crash loses at most the events since the last flush.
