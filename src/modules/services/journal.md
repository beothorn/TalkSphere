# Authoritative coordinator image journal

Journal binds one local identity, Authority, finite Limits and exclusive Persistence
adapter. open requires Register for explicit absent-store initialization, or Recover
for an existing image. Existing state is never overwritten by initialization and
wrong identities, missing existing records and oversized images fail explicitly.
The runtime owns and validates the private operation schema behind opaque payload
bytes before admitting any recovered operation.

read returns an independent image copy only under Observe, Coordinate or Recover
authority. replace requires Coordinate, compares the complete expected old image
and atomically persists the complete candidate before publishing visibility. A
stale candidate conflicts. Storage leaves old visibility intact. Unknown gates
all dependent reads/writes until recover reloads and synchronizes authoritative
bytes under Recover authority. Recovery never repeats the uncertain write.

Image compare-and-replace prevents concurrent writers from discarding another
operation's state. The coordinator must additionally serialize participant side
effects and validate immutable decisions; this low-level journal does not decide
commit/abort or authorize handler execution. Required operation schema and real
participant coordination are built on this boundary in later plan steps.
