# Durable commit contract

Record contains exact publisher bytes, complete original Board JSON and definition
JSON indexed by ID. from_prepared copies a complete validated candidate; prepare
revalidates loaded bytes under current limits and capabilities before visibility.

Persistence.load returns a complete committed record or explicit absence; corruption
is not absence. commit atomically replaces a whole record. Acknowledgement follows
all required synchronization. Storage means precommit failure; Unknown means the
write may have committed and requires load/recovery, never assumed rollback.
Memory is explicitly volatile and provides no process-restart guarantee.

Provider.open loads and validates a record, rejecting wrong publisher and unsupported
current capabilities before reads. Provider.publish persists before replacing
visible memory. Unknown gates subsequent current reads and writes until recover
loads and validates durable state. recover requires Publish authority, does not
retry the write and keeps previously pinned bytes immutable. Precommit failures
leave current state unchanged. Local record formats add no peer wire fields.
