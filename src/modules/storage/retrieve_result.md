# Complete retained retrieval output

Limits separately bounds complete encoded result bytes, whole-object integrity
scan bytes and logical output values. reservation checks exact fixed result
shape overhead plus accepted data length before allocation/I/O, returning one
pending operation and capacity for both the logical output encoding and retained
result buffer. The current local codec overhead is 181 encoded bytes and 114
logical metadata bytes; tests fix these accounting assumptions.

complete consumes an accepted Pinned reference and Limits. It scans the retained
immutable object with bounded chunks, hashes that same sequence and captures only
the authorized range. Whole-object mismatch, short/missing reads or bounds fail
without returning partial output. The internal scan returns no bytes outside the
accepted range. Its maximum must cover already accepted lease sizes; it is local
provider resource policy rather than the current offering's maximum store size.

Output contains retrieve, original lease ID/offset, exact range data, original
whole-object hash and the frozen accepted work price. Completion owns the logical
output, exact local encoded result bytes and a NoChange metadata effect. Local
encoding is not the Peer Protocol wire format. It does not charge or decide.

stage uses the original pin key and unambiguously framed accepted context/range
binding. It retains a newly completed result through Journal.stage; an identical
existing local record returns Journal.completion without rereading backing data.
Changed scope conflicts. This is local durable result recovery, not a generic
cross-connection retry/status operation. Unknown staging preserves uncertainty.

Call only through trusted owner admission after reserving the returned Usage;
Services owns preparation, commitment and accounting. Byte/result references must
outlive consumers. Provider paid lease creation and full network registration
remain incomplete while upstream issue #11 is unresolved.
