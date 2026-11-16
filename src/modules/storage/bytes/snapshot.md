# Atomic owner metadata persistence

The Linux byte Store also supplies the Storage Persistence port, sharing its
exclusive writer lifetime and protected directory. load takes a finite maximum,
checks file size before allocation, reads only installed metadata and synchronizes
the directory before returning. Absence differs from an empty image.

replace writes a private exclusive temporary file, synchronizes it, atomically
replaces current metadata and synchronizes the directory. Before installation,
failure is Storage; after possible installation, failure is Unknown. load recovers
the installed image and removes unacknowledged temporary metadata without promoting
it. The authoritative owner serializes updates and validates Image schema/checksum;
this platform adapter never interprets leases or coordinator decisions.

Unsafe entry modes/types/link counts and symlinks fail closed. Limits are enforced
on load before proportional allocation. The caller bounds images before replace.
A local commit does not prove remote receipt or authorize an abort/refund.
