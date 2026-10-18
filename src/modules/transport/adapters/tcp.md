# TCP provider

`TcpProvider.new` starts one readiness worker and returns an OS error if setup
fails. `clock` creates a SystemClock sharing that worker. `connect(endpoint)`
starts a nonblocking attempt; polling resolves actual completion/error and
transfers its socket exactly once. Dropping a pending attempt cancels it.
`listen(endpoint)` binds a listener; port zero requests an OS-selected port.

Listeners report their bound address, accept one ready socket per successful
poll and close idempotently. Sockets perform partial reads/writes, return Pending
on WouldBlock with a registered wakeup, preserve OS errors and return zero on
read EOF. Explicit close and Drop release both socket directions and readiness
registration. No authentication is implied by TCP success.
