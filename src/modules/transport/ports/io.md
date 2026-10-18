# Nonblocking platform ports

Socket.poll_read fills a caller buffer and returns a byte count, EOF (zero),
error or Pending; poll_write permits partial writes and reports errors/zero
progress. Pending registers a task wakeup. Socket.close stops both directions;
Drop also releases ownership. Connecting.poll_connected transfers one successful
socket or reports failure; dropping the attempt cancels it.

Listener.local_addr reports the actual bound endpoint; poll_accept transfers one
socket at a time; close/Drop stop listening. SocketProvider.connect initiates an
outgoing attempt without blocking; listen binds an endpoint, including port
zero for OS assignment. Socket errors remain typed OS errors.

Clock.now returns monotonic process-local time. poll_deadline replaces its prior
timer/waker and becomes ready at/after the supplied deadline. The owning driver
must select the earliest timer across its work and use checked time arithmetic.
All platform ports are owned by one polling driver and can move between threads.
