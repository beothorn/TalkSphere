# Requester application catalog retention

Catalog is an application-owned sink for validated successful leases. retain
should durably preserve independent provider/ID, content/hash, times and original
accepted terms, accept exact repeated entries and reject conflicting ID reuse.
It does not receive plaintext encryption keys through these methods.

Committed supplies the local requester identity and original complete acknowledged
Invocation/Completion for one OperationKey. Services adapts the public
Coordinator.completed observation using an authorized Observe Context; no private
journal data is decoded, and provisional results remain unavailable.

remember observes only that original committed result, checks key/requester
correlation, independently validates the store output against original input and
clock policy, checks the completion's exact work price and then calls Catalog.
It returns the validated Lease after successful retention. Missing, uncommitted,
unknown, unauthorized or malformed results never reach the catalog.

A catalog failure does not reverse committed work, credits or trust. Repeating
the original local observation can retry catalog persistence without creating a
new peer store. Independent provider replicas remain separate catalog entries;
this port neither creates replicas nor promises that one lease is a backup.
