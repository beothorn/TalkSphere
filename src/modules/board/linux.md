# Linux durable store

FileStore.open validates Limits, traverses each directory component with no-follow
descriptor anchoring and creates only the final missing directory. The store must
be owned by the current user with mode 0700; regular files must have mode 0600,
matching owner and one link. An exclusive writer.lock is retained until drop.
Parent traversal, symlinks, insecure objects and concurrent opens fail explicitly.

Persistence.load reads the bounded complete board.bin record, verifies its private
format and synchronizes the directory before recovery. Absence is None; corruption
fails and never generates a replacement. commit writes board.pending exclusively,
syncs the file, atomically renames to board.bin and syncs the directory before
acknowledging. Failure after rename is Unknown. Precommit failure removes staging;
startup removes an abandoned protected staging file without promoting it.
The private record preserves publisher and exact documents, with checked lengths,
counts, duplicate rejection and no trailing bytes. Content is revalidated by the
Board owner before becoming live. Files are permission-protected, not encrypted;
these are public documents, separate from Identity keys and paid Storage state.
This adapter is available only on Linux. Other targets have no durable adapter.
