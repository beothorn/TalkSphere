# Atomic identity installation

FileStore implements KeyStore with scoped Administration and CredentialRecord.
install checks authority and record consistency, writes both pairs to protected
identity.pending, syncs its contents, atomically renames without replacing an
existing identity and syncs directory metadata before durable success. The
exclusive writer lock serializes access. Existing identity state returns Conflict.

Failure before publication removes staging and never creates a usable partial
pair. Failure after publication is OutcomeUnknown, not assumed rollback. reconcile
revalidates the existing complete credential and syncs directory durability without
regenerating keys; the caller can then compare the expected identity. open reads
through the protected store contract and never silently repairs missing state.

Restart cleanup runs under the exclusive lock and removes only known protected
identity.pending/contacts.pending files after owner, type and mode validation.
Unexpected or insecure staging fails closed. Removing a file does not promise
physical disk erasure; the store relies on OS protection and documented platform
sync/rename guarantees. Private in-memory staging remains zeroizing.

Contact snapshots use the same protected atomic publication machinery with
replacement explicitly permitted only for contacts.bin; that adapter is separate.
