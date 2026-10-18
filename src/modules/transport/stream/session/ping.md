# Session probe operations

`ping(token, timeout, now)` queues an eight-byte token and records its monotonic
start/deadline. Time comes from the driver's injected clock. The timeout must be
positive and representable; outstanding tokens must be unique. At most 64 probes
are pending. Closed sessions, invalid requests and capacity exhaustion return
local errors without queueing probes.

`tick(now)` expires deadlines at or before now, emitting one failure per token.
`next_deadline` returns the earliest timer or no value. The driver registers
that deadline with the clock. Verified PING receives one identical PONG. PONG
matches only an outstanding token and reports elapsed time; unsolicited,
duplicate and late expired responses are ignored. Connection closure fails
remaining probes. Fresh caller-selected tokens avoid ambiguity with delayed
responses to previously completed probes.
