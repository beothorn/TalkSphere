# Protected Linux Services store

FileStore.open(path, limits, create) opens only an owner-owned 0700 directory,
walking every component with anchored no-follow descriptors and rejecting parent
traversal. create permits creation of the last directory only; false never creates
the directory. A retained exclusive writer.lock prevents independent same-identity
writers in this store. Files are owner-owned regular 0600 files with one link.

Load returns a complete bounded identity-bound Record or true absence. Invalid
framing/version/length, insecure metadata and I/O failures never become absence.
Commit writes an exclusive staging file, synchronizes it, renames atomically and
synchronizes the directory before acknowledgement. Failure before rename is known
uncommitted; failure after rename is Unknown and requires load/status recovery.
Protected abandoned staging is deleted, never promoted. The format is private,
versioned, length-delimited and unrelated to CR2SE peer formats or paid Storage.

Fault tests simulate every write/synchronization boundary; they do not demonstrate
physical power-loss safety. Other processes under the same user or root remain
outside the protection model. Unsupported platforms have no FileStore adapter.
