# Coordinator lifecycle

Phase describes Accepted, Reserved, Executing, Staged, Prepared, Commit, Abort,
Committed and Aborted. Accepted retains consent before reservation; Executing
retains an execution-attempt marker so recovery never reruns external effects.
Commit/Abort are immutable decisions requiring delivery; terminal states require
all local effects/result retention or release to be durable. Payment and remote
receipt remain independent observations.

The owner-private versioned image retains exact key/roles/issuer, accepted quote,
Board and definition bytes, publisher/direction, pricing/bound/fees, metadata,
input, complete staged output/result/effect and participant tokens. Recovery checks
exact kinds/widths, duplicate keys, limits, phase prerequisites and price bounds.
Image format is local and does not define peer wire behavior.

The development coordinator payload now uses private schema version2 to retain
check purpose, stable entitlement, response interval and clock-domain observations.
Old alpha version1 payloads are explicitly Unsupported; no missing/corrupt image is
reinitialized as recovery. Outer protected-file framing and local call contract
version1 are unchanged; no CR2SE peer encoding/version changes are introduced.
