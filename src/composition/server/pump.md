# Wakeable event polling

Signal is cloneable local runtime control. notify retains a wake even before wait;
shutdown retains a shutdown request and wakes the idle runtime; stopping reads it.
wait takes a finite timeout and waits until a notification, shutdown or that timeout.
Spurious condition-variable wakeups do not fabricate readiness. Synchronization
failure returns Runtime. These are local controls, never peer-authenticated commands.

Pump owns TransportControl and Signal. turn calls the consumer for at most 64
observations, outside protocol internals, then returns the count. A full turn must
be followed by another turn before sleeping; a partial turn has reached Pending
with registered task wakeups. Unexpected driver termination or consumer failure
returns Runtime/the consumer's Failure. The application interleaves lifecycle
checks between turns and supplies finite waits for shutdown deadlines. No executor,
fixed polling sleep or global signal handler is installed by these contracts.
