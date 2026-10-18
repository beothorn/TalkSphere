# Fair session scheduling

`pop_outbound` selects whole frames round-robin across ready stream IDs,
including connection control. It never reorders a stream's own frames. OPENs
remain in allocation order across streams, and GOAWAY cannot overtake earlier
queued OPENs. The connection writer completes its current frame before asking
for another, so partial writes cannot interleave frame bytes.

`pop_stream_event(owner)` selects the oldest event authorized for that consumer,
preserving its stream order while another consumer is blocked. Global connection
events remain available through `pop_event`. Per-stream buffering limits isolate
slow consumers; no wire flow-control frame is introduced. Metadata and byte
budgets remain finite even when no consumer makes progress.
