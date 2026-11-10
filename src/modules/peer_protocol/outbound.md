# Outbound record ownership

Outbound.new takes a positive maximum aggregate queued byte count. Enqueue checks
record validity and complete framed size against remaining capacity before
allocating serialized bytes; refusal leaves previous records unchanged. Front
borrows at most the requested fragment length without advancing; zero requests
or an empty queue return no bytes. Backpressure must repeat the same borrowed
fragment, never silently remove it. Advance acknowledges only a positive prefix
of the current record after actual local Transport admission; excessive/zero
counts are Invalid without mutation. Pending reports all remaining queued bytes.
Cancel discards unsent bytes and resets queue capacity; it proves nothing about
remote receipt, previous sends or committed effects. The owning exchange controls
END/CANCEL and coordinates cancellation separately. No hidden network retry occurs.

Available reports remaining framed-byte capacity for bounded chunk planning before allocation.
