# Bounded dependency execution

`Pool` transfers synchronous Board, Services or Ledger calls from composition to
fixed worker threads. It never receives peer JSON as an executable operation.
The reactor submits an owned `Job`, obtains a unique process-local `Ticket`, and
polls correlated `Completion` values using its normal task waker. Completion
contains the exact dependency result; it does not imply a participant committed.

Construction accepts 1–64 workers, no more workers than outstanding capacity,
and at most 4096 outstanding calls. Zero/impossible limits are invalid; OS thread
creation failure is unavailable. Capacity includes running calls, queued calls and
completed results that the caller has not consumed. Captured input and result
bytes require separate admission budgets at the engine boundary.

Submission never waits for a worker. Refusal returns both the typed error and the
original, unexecuted job. Ticket numbers never wrap or identify a later operation.
Polling registers the observer before checking the completion queue, so completion
between those actions cannot lose its wakeup. Callbacks execute without holding
the queue mutex, and independent workers can progress around a blocked callback.
No callback executes on the submitting or polling reactor thread.

`stop` closes new admission; accepted calls still run and remain observable.
`outstanding` counts results still owed to composition. Normal shutdown stops
admission and consumes every outstanding completion. Dropping the pool detaches
workers rather than blocking the reactor; accepted owner effects can still occur.
Dependencies must eventually return; there is no forced termination of a thread.

Timeout, cancellation, lost observers and panics never authorize a retry, refund
or abort. A worker panic becomes `Unknown`, since an owner may already have
committed, and the worker continues with later calls. Expected errors must use
normal typed results. Durable recovery belongs to the original module owner.
