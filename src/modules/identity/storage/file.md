# Protected Linux credential reads

FileStore.open(path, limits) opens an owner-only directory and returns the store
and its local administrative capability. Trusted composition supplies the path;
peers cannot select it. Each directory component is opened relative to the previous
descriptor without following symlinks. Parent traversal and missing parent paths
fail. Only the final missing directory may be created, with mode 0700.

The final directory must be owned by the effective user and exactly mode 0700.
A mode-0600 regular writer.lock file is locked exclusively without blocking;
contention returns Conflict. The lock remains held until the store drops.
Safe rustix calls preserve the project's no-unsafe-code policy.

read checks administrative authority before inspecting identity.bin, then opens
it relative to the retained directory with no-follow/nonblocking flags. The file
must be an owner-owned mode-0600 regular file with one hard link. Size is checked
before bounded reads into zeroizing staging; a growing file cannot exceed the
budget silently. CredentialRecord checks the complete record shape; callers must
also decode it before treating it as a validated identity. Missing files never
cause replacement keys to be generated. Errors are fixed categories without paths.

Protection relies on filesystem permissions, not encryption, and does not defend
against the same account or root. This step does not yet install credentials.
