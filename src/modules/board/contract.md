# Direct Board boundary

Boundary construction consumes trusted Authority and validates Limits without
filesystem or network effects. Invalid configuration fails before construction.
Limits returns the immutable policy by value.

Context carries a borrowed grant, monotonic deadline and cancellation observation.
Check rejects cancelled or expired calls before validating publisher, operation
and bytes against the receiving authority. Concurrent cancellation delivery is
the adapter's responsibility; this boundary does not spawn a worker.

Read takes Context and publisher ID, checks Read authority, then currently
returns Unsupported. It never manufactures an empty publication.

Snapshot is owner-created and opaque: callers cannot construct validated state.
It exposes the bound publisher and borrowed exact immutable bytes; clones pin
content independently of subsequent publication. Association with a publisher
alone is not evidence of direct authenticated retrieval.

Publication, definition lookup, cache and selection operations will be added in
their implementation steps. Their intended rules are in the plan and acceptance
inventory; this initial contract does not claim they exist.

## Logical implementation ports

BoardPorts declares the future direct-call operations, with implementation-owned
validated Candidate, Definition, Retrieval, SelectionRequest and Selection types.
Publish consumes a complete candidate and returns a snapshot, preserving explicit
Storage versus Unknown commitment failures. Definition reads one exact offering
ID in a pinned snapshot. Cache accepts a trusted adapter's retrieval association.
Select returns immutable terms/usability for caller-specified requirements; it
never executes or pays. Each receives Context and enforces its own action.
No implementation of these ports is supplied by the scaffold. Local contract
version is 1, independent of JSON schema version and peer protocol version.
