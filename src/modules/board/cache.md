# Remote cache

Cache.new takes trusted authority and finite Limits, performing no fetch.
store checks Cache authority for the asserted source identity, parses a complete
Board, assigns injected monotonic receipt/expiry seconds and atomically installs
an immutable Remote receipt. Trusted adapters supply Direct(authenticated publisher)
or Forwarded(forwarder); document fields and identical bytes cannot authenticate
an original publisher. The grant is the authority to supply that provenance.

Entries are keyed by source identity and direct/forwarded association, never TCP
endpoint. Expired entries may be removed during insertion. Entry capacity, live
receipt revision capacity and checked expiry arithmetic are explicit finite policy;
exhaustion returns Limit without evicting valid entries. read requires Read scope,
checks freshness and rejects backward-time/expired receipts as Stale without any
network refresh. Remote exposes original bytes, provenance, receipt/expiry and
validated definition count. Pins preserve bytes but never promise indefinite freshness.
