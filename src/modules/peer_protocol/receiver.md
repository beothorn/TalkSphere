# Ordered detached body receiver

Receiver.new takes validated Pending metadata, admits only its already bounded
body declarations and creates no proportional payload allocation. Push accepts
only Body records for the next declared ID and exact contiguous offset, with
1..65536 data bytes. It uses Services Body through its public contract. A body
finishes before the next ID begins; complete text validates UTF-8 after all bytes,
so UTF-8 sequences may span records. Wrong kind/order/length, unknown ID, extra
bytes or invalid text poison the value and discard provisional buffers. Later
pushes return Conflict. No provisional Blob or typed value is exposed.

Complete reports whether every declared body is finished. Finish consumes the
receiver and returns one exact assembled Value; failed/missing data is Invalid.
Inline-only values require no body record. Dropping the receiver releases its
provisional buffers. Content hashes and family semantics are independently
validated by Services; this layer does not acquire spending or execution rights.
The exchange must still require input.end/result.end and normal direction END;
body completion alone never proves a successful invocation or remote receipt.
