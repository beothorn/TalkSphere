# Durable configured contacts

FileStore.read_contacts(administration) returns a copied list of validated contacts
in binary-ID order. It requires this store's administrative capability with the
configured record-byte budget before inspecting existence or contents. A missing
record is Unavailable; callers explicitly initialize a new book with an empty
snapshot instead of treating every read failure as an empty book.

FileStore.replace_contacts(administration, contacts) validates the complete list,
rejects duplicate IDs, invalid endpoints and exceeded contact/text/record limits,
then publishes the snapshot atomically. The exclusive store lock prevents concurrent
writers. Existing returned snapshots are independent copies; updating an endpoint
retains its expected ID. This never accesses or replaces identity key records.

The implementation-specific local format is ASCII `TSCON001` followed by a newline
and zero or more canonical contact-text lines, each terminated by a newline. It is
not a CR2SE wire format. Canonical spelling, version, limits and duplicate IDs are
checked on every read; malformed records are not silently repaired. Files use the
same owner-only, no-follow protection and anchored directory as credential storage.

Success means the staged file and containing directory were synchronized. Failures
before installation preserve the prior snapshot. OutcomeUnknown means the new
snapshot may have been installed; reconcile_contacts(administration) validates the
currently installed snapshot, synchronizes the directory and returns that snapshot.
It does not choose a retry on the caller's behalf. A reconciliation sync failure
remains OutcomeUnknown. No Discovery, trust or paid Storage state is implied.
