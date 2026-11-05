# Controlled shutdown

Transport forwards the TransportControl port but refuses new connect/listen/accept/
OPEN requests while Stopping. observe retains incoming local connection authority,
removes completed connections and treats unexpected listener loss while Ready as
Network failure. Lifecycle events carry no private keys or inferred service result.

begin_shutdown is idempotent after Ready: stop the listener, establish a finite
monotonic deadline, drain established sessions and close incomplete handshakes.
shutdown_progress returns false while work remains, true when handles have closed,
and ShutdownTimeout on deadline or forced closure. Caller drops the driver before
reporting Stopped so socket, worker and protected-store ownership are released.
A connection closure never rewrites previously completed stream outcomes.

Signal.shutdown requests normal shutdown the first time and forcing on repetition.
Signals.install forwards SIGINT/SIGTERM from signal-hook's iterator thread; raw OS
handlers do not execute blocking protocol work. Dropping Signals closes and joins
that helper. Registration/startup failure returns Runtime before socket startup.
Application tests can inject Signal directly without installing global handlers.

All Connection admissions through the server's TransportControl port retain their
local handle for shutdown, including callers beyond startup contacts. Incoming
observations deduplicate already retained handles. A real TCP drain regression test
covers this path; previously only startup contacts were tracked, allowing a direct
outgoing connection to remain open after its streams completed.
