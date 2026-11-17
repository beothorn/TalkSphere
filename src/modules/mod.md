# Protocol module exports

Transport exports framing, state, dependency ports and native platform adapters.
Cryptography exports real primitives, bounded envelopes and integrity adapters.
Identity exports canonical IDs, protected key authority, credentials and recovery.
Board exports validated publication, immutable snapshots, Linux durability,
attributed caches, explicit selection and local document tools.
Ledger exports finite issuer-specific accounting, durable reservations/settlement
and authenticated control contracts. Services exports typed handler registration,
validated metadata-aware pricing and durable coordination through public owners.
Peer Protocol exports bounded wire values, body/exchange handling and explicit
authenticated runtime composition. Storage exports validated assessment and
durable participant/admission primitives; paid lease creation and a complete
Storage provider remain unfinished.

Modules use explicit public contracts; composition selects adapters and grants
trusted authority. Exporting a module neither opens a listener nor advertises a
capability. Remaining roadmap modules must not be replaced by fake providers.
