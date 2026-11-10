# Evidence without provider correctness claims

Sender.new requires original receiving authority, separate Services Observe grant,
real Coordinator, opaque Dispatched context and finite limits. Only acknowledged
check commitment can yield evidence. Original quote/roles/price must match.
Standard Storage and Messaging evidence binds 32-byte resource ID, uint64 offset
and one-byte value; Discovery binds 32-byte index/record IDs and exact retained
record bytes. Custom families need an explicitly installed evidence schema.
Provider outcome fields are forbidden. Family Rules independently validate original
challenge/content association. This binding does not install any standard provider.

Sender queues result metadata with exact finalPrice, detached evidence bodies,
result.end and then permits composition to send END after drain. queued is only
local queue admission; evidence/association borrow retained data and original route.
Failed sends retain the original committed result and possible accounting effects.

Receiver.new binds a reserved Outgoing, incoming Invoke authority, finite limits
and trusted Clock. control permits one evidence result and result.end after exact
bodies, with original exact check price and no provider outcome. body accepts only
ordered declared data. No provisional bytes become successful evidence.
finish requires normal original-stream completion, exact standard widths and a
trusted local receive stamp, then Services independently validates timing and
family evidence before staging. Only later committed check_outcome consults local
private expectations. Complete wrong evidence may be paid fail; missing/late or
malformed evidence cannot become a requester charge. Explicit failures require
normal completion before abort; abnormal closure remains Unknown.

The existing paid-check response-loss ambiguity is tracked by
[CR2SE issue10](https://github.com/beothorn/CR2SE/issues/10). A requester-local timeout
does not reverse a provider's prior durable commitment or establish globally equal
credit views. No receipt, refund or invented settlement protocol is implied.
