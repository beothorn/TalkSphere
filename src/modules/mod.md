# Protocol module exports

Transport exports framing, state, dependency ports and native platform adapters.
Cryptography exports real primitives, bounded envelopes and integrity adapters.
Identity exports canonical IDs, protected key authority, credentials and recovery.
Board exports validated publication, immutable snapshots, Linux durability,
attributed caches, explicit selection and local document tools.
Services currently exports only its reusable schema-format validator for Board;
invocation, handlers, coordination and payments remain later implementation work.

Modules use explicit public contracts; composition selects adapters and grants
trusted authority. Exporting a module neither opens a listener nor advertises a
capability. Remaining roadmap modules must not be replaced by fake providers.
