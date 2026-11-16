# Durable bounded byte staging

Linux Store.open takes a locally configured path and explicit create flag. It
walks components without following symlinks, checks final directory ownership
and mode 0700, and holds an exclusive writer lock. Only the final component may
be created. Protection excludes other users, not the same account or root.

begin takes an owner-issued 32-byte object ID, positive declared length and a
finite authorized maximum. Oversized lengths fail before proportional allocation.
It creates a private exclusive provisional entry; no peer text becomes a path.
Upload.append accepts only the next offset within that length. Invalid order,
excess bytes or failed writes poison the attempt. Incomplete/corrupt uploads never
produce a complete Handle. Dropping an uninstalled upload best-effort removes its
provisional entry; crash orphans require explicit discard after owner recovery.

finish checks exact length and SHA-256, synchronizes bytes, atomically renames
with no replacement, then synchronizes the directory. It returns an immutable
Handle only after these barriers. A completed object is not an active lease:
the metadata participant separately decides that obligation. Conflict prevents
ID reuse from overwriting existing objects. Failures before installation return
Storage; failures after possible installation return Unknown and preserve bytes.

reopen takes trusted object ID and exact expected length. It rejects symlinks,
unsafe modes/types/link counts and size mismatch. Metadata recovery must also
verify integrity before serving a lease. discard removes only provisional entries
and synchronizes the directory; it is not automatic abort authority.

Handle.id and len expose opaque identity and validated length; is_empty is false
for supported objects. read takes a positive range and independent allocation
maximum. It validates by subtraction before allocating and uses exact positional
reads, so clones have no shared cursor. Invalid denotes out-of-object ranges,
Limit excessive allocation, and Storage missing/short backing bytes. Open handles
retain bytes through unlink and keep the owner directory/lock alive.

This local adapter writes bounded chunks, but the existing Peer Protocol/Services
Value path still buffers finite complete bodies. Advertising limits must respect
both that memory budget and local byte quotas. No API grants requester authority,
creates leases, authorizes payment, encrypts or replicates data.
