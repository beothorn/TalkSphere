# Versioned Storage metadata images

Image contains the authoritative provider ID, acknowledged retention Anchor,
committed Lease descriptions and Retained operation records. Each byte object ID
equals its lease ID; terminal lease history survives cleanup to prevent ID reuse.
Retained contains original OperationKey, a digest binding accepted input/terms,
complete Completion and family-local Stage: Staged, Prepared(token), Committed
or Aborted. Committed retains its exact preparation token; Aborted retains one
when preparation happened, allowing duplicate decisions to validate original
tokens after restart. This adds no Services status value or peer message. All blobs belong to the owner,
independent of sessions. Participant mutation and recovery decisions remain later
owner operations, not permissions granted by this codec.

Limits independently bounds complete encoded bytes, lease and operation counts,
and each logical output's Services Budget. encode validates unique IDs/keys,
provider ownership, positive size/price, increasing times, nonempty accepted terms,
terminal/active states and nonempty preparation tokens. It writes TSSTORE1 framing,
exact integer fields and a SHA-256 corruption checksum. Logical outputs use the
public local Services value codec, never another owner's journal format.

Decode takes a complete bounded image, expected provider ID and Limits. It checks
checksum/version, counts before collection growth, exact lengths/types, complete
consumption and the same owner invariants. It never silently resets corrupt or
future data. Foreign owner is Denied; malformed images are Invalid; exhausted
bounds are Limit. The checksum detects accidental corruption; it authenticates
neither an attacker nor a filesystem owner.

On Linux, verify_active opens every active byte object and checks exact size and
whole-object hash using at most 65536 bytes per read. Terminal lease descriptions
may outlive physical objects. Missing, short or corrupt active bytes gate recovery;
the caller must retain obligations and report failure rather than drop records.
This check neither reexecutes work nor reconciles accounting.
