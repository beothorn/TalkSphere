# System monotonic clock

`SystemClock.new` creates a readiness worker and one timer registration,
returning OS/thread initialization errors. Provider.clock can share an existing
worker instead. `now` returns process-local monotonic time. `poll_deadline`
returns ready at/after the given deadline; otherwise it replaces the previous
deadline/waker and returns Pending. Dropping the clock cancels its timer.
