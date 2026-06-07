# Shared storage file system

This folder isolates file-system work for shared storage.

It does not know about SQLite rows, expiration policy, peers, credits, or networking. It only prepares directories, builds safe file paths, writes bytes, reads bytes, and deletes files.

The public shared-storage module calls this code before or after metadata work so the operation order stays visible in `shared_storage.c`.
