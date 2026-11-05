# Executable runtime loop

run owns the composed Transport, explicitly registered Router and local Signal.
It binds the listener, reports Ready with the actual address/public ID, then starts
one-shot contacts. Contact admission failure does not terminate listener operation.
It polls bounded turns, updates owned lifecycle state, reports authenticated public
peer context and routes stream events through the owner-checked Router. Pending
waits use notifications and finite lifecycle timeouts; no fixed polling sleep is used.

Status contains Ready, Authenticated, ContactFailed, ConnectionClosed and Stopped,
never payload bytes or private key records. Reporting is a trusted nonblocking local
callback. Authenticated is emitted only from the Driver's verified context; a socket
or admitted contact is insufficient. ConnectionClosed never rolls back application
state or asserts a service result. Production can use an empty router.

Shutdown stops admission and drains to the configured deadline, then drops the pump
and driver before reporting normal Stopped. Timeout/failure drops owned resources
and returns a typed Failure without claiming graceful completion. A signal received
before startup releases the unstarted composition without binding a listener.
