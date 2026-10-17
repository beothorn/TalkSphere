# FrameWriter

Default creates an idle writer. start takes ownership of a complete WireFrame
and its direction's integrity phase. It validates tag presence and admits only
one frame; Busy, Shape and Closed reject work. The caller retains ownership of
logical requests until successful admission rather than treating rejection as
network delivery. The method consumes its frame argument even on rejection.

poll_flush takes the same socket and polling context repeatedly, preserving
accepted byte offsets until the entire frame has been sent. Pending arranges a
wakeup; an empty writer returns success. Zero writes or socket failures latch
the writer closed and discard queued bytes. is_busy reports unfinished bytes.
No delivery acknowledgement or peer processing is implied by a successful flush.
