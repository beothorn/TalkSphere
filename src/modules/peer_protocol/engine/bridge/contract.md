# Stream scope, handler and budget contracts

`Scope` carries the local and verified peer identities, a unique connection
instance, the Transport-issued nonzero stream number and the original monotonic
admission deadline. These observations are
supplied by trusted composition, never reconstructed from application JSON.
`association(requester)` creates an operation association only for one of those
two endpoints. The handler must first validate the requester against selected
provided/wanted terms; choosing an endpoint alone grants no application authority.

`Factory.open` receives the scope, local/remote opener direction and a bounded
output queue. It creates one stream handler, optionally queuing an explicitly
requested local opening control. Remote input cannot install a factory or handler.
A factory error terminates that stream without implying economic rollback.

`Handler.record` receives one complete framed control or detached body. The
handler owns control parsing, Exchange ordering, exact original-stream binding,
local authority and all Board/Services/Ledger semantics. Framing is not service
admission. `remote_end` runs only after complete record framing; the handler must
also require application terminal markers before exposing a result. `writable`
allows incremental producers to refill a drained queue; it acknowledges local
Transport queue admission only. `closed` receives the actual Transport outcome,
including closure observed while a dependency was still running. A handler must
preserve its own semantic failure state and resolve accepted effects through the
original module owners, even when Transport itself completed normally.

All callbacks execute on dependency workers, sequentially for each stream. Typed
errors stop fresh input callbacks and terminate only that stream. Closure still
reaches an existing handler; there is no automatic retry or presumed refund.
Callbacks must preserve the original scope deadline in module contexts rather
than restart it after worker queue delay. Expired fresh observations are refused;
closure cleanup remains permitted. Callbacks must eventually return, retain finite state and avoid payloads/secrets
in diagnostics. Handler application state and retained domain results require
separate Services/family budgets.

`Output.enqueue` admits a whole validated record only if its encoded bytes fit;
refusal preserves already queued bytes. `end` requests local END after those
bytes drain. Enqueue after that request is a conflict. `available` reports remaining
framed-byte capacity so producers can bound allocations before construction.

`Config` requires positive connection/exchange, input/output, memory, control-rate
and lifetime bounds. Exchanges are at most4096; workers are1–64 and no greater
than the exchange count. Output capacity must fit a1048576-byte control plus its
five-byte header. Lifetime uses monotonic local time and must be representable.
Arithmetic overflow and insufficient aggregate memory are invalid configuration.

Before admitting each stream, the adapter reserves twice the maximum control/body
record storage, the complete per-stream input budget and twice the output budget.
The aggregate memory budget can therefore reduce concurrency below the exchange
count. Pending input remains charged while a worker owns it. At most64 queued
observations are retained per stream. Control rate is a per-stream, one-second
monotonic processing window; aggregate/global policy is a separate composition
obligation. These are conservative logical byte reservations, not an RSS bound.
