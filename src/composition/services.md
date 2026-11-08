# Service publication lifecycle composition

Publication owns a Board Provider by value, the matching local Registry, Board
limits and one composition writer gate. new checks local registry identity and
limits. All publication writers must use this facade so a read/prune/write cannot
discard another publication update.

publish validates candidate identity and current registry capabilities before
calling Board's public atomic publication contract. read/select/current expose
only immutable public Board contracts and preserve authority/staleness failures.
No private Board tables or locks are used.

drain validates registration authority, reads a complete Board snapshot, removes
matching service/version offerings from both lists, retains every other exact
definition and durably publishes the complete candidate before disabling registry
selection. Accepted work retains its original implementation pin and immutable
terms. A failed/uncertain publication leaves the handler active; if later registry
authority is revoked, an unadvertised active implementation is safe to retry.
recover reloads the authoritative Board image under the same writer gate before
retrying lifecycle changes. Errors preserve invalid, unsupported, stale/conflict,
authority, resource, cancellation and known/unknown storage outcomes.
