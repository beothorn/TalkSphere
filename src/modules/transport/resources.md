# Shared Transport resources

`Resources.new(limits)` creates finite connection and outbound-byte counters
using validated local limits. Clones share counters. `limits` returns policy;
`connections` and `outbound_bytes` expose current occupancy without payloads.
All connections in a Transport instance must share the same Resources object.

Admission reserves capacity before allocating protocol state. Outbound frames
retain serialized-byte reservations through queueing, integrity sealing and
partial writes. Only final output completion, discard or closure releases them.
Checked arithmetic prevents overflow. Dropping one ownership clone does not
release a reservation retained elsewhere. There is no wire flow-control change.

Stream state enforces configured active-stream counts and per-stream buffered
bytes. Consumer event counts are also finite. Local saturation returns capacity
without consuming an OPEN ID; remote resource refusal consumes its valid next
ID. Cancellation releases queued stream output before admitting its control
frame; if no global control capacity remains, the connection closes with a
resource failure. Failed refusals likewise escalate to connection closure.
