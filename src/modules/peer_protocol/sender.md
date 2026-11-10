# Detached body sender

Sender.new takes immutable Blob sources, exact validated Descriptor declarations
and Limits. It validates sequential IDs, positive lengths, source/declaration
count equality, exact source sizes, aggregate body/byte budgets and final UTF-8
where text before any queue operation. No source is opened from a path or fetched
over the network. The sender retains immutable ownership independently of a
client session or borrowed application input buffer.

Enqueue_next takes an Outbound queue and positive maximum chunk size. It admits
at most one body record in ID/offset order, bounded to65536 data bytes and the
queue's remaining framed capacity. Source progress changes only on successful
local queue admission. Backpressure is Limit and leaves the source position
unchanged; zero chunk is Invalid. False means all sources are queued. Queued
reports that local condition, not Transport flush, remote receipt or committed
service success. The exchange separately queues terminal markers and END.
Dropping/cancelling sources triggers no network work retry or financial rollback.
