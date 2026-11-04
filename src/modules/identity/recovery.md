# Protected backup and recovery boundaries

Identity.backup takes a loaded source handle, its purpose-scoped Backup capability,
a protected destination store and that destination's administrative authority. It
checks handle ownership, source authority for the complete 232-byte record, and
destination authority before staging private data. Both independent private pairs
and their validated public association are installed as one atomic record.

Only a protected KeyStore may receive the guarded record. The tested Linux FileStore
uses owner-only permissions, anchored no-follow paths, an exclusive writer lock,
no-overwrite installation and file/directory synchronization. Existing destinations
return Conflict. Pre-install failure cleans staging; OutcomeUnknown requires the
destination's reconciliation operation, not automatic backup retry. Backup returns
no private bytes and writes neither stdout nor peer messages. The live source and
its public snapshot remain unchanged on success or failure.

The TSID0001 record is a documented local format, not a portable CR2SE export
standard. It is permission-protected, not encrypted at rest. Secure an independent
backup location and any later copies; moving this file to an unprotected destination
removes its access protection. A holder of both private keys can impersonate the
identity and decrypt its encrypted objects. Backup does not capture contacts,
Ledger, Board, provider/lease catalogs or service commitments.

Identity.grant_import(destination administration, expected binary ID, byte budget)
issues purpose-scoped Import authority before an empty destination has a handle.
Identity.restore takes destination administration, that capability, the expected
ID, a protected source store and its administration. Both boundaries are checked
before reading private records. Restore validates both pairs and every cached field,
requires the expected ID, then installs without overwrite. It consumes no entropy
and returns a usable handle only after durable success. Failures drop temporary
private authority. An uncertain installation must be reconciled through the store.
Existing identities conflict even when their keys match; restoration never implies
rotation, replacement or recovery of independent application state.
