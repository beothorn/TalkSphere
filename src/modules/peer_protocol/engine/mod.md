# Peer Protocol engine primitives

The engine boundary connects authenticated Transport observations to bounded
asynchronous module calls. Its [worker pool](worker.md) is available; stream
routing and production composition are still under development. These primitives
alone do not register `cr2se.peer`, install a handler or establish conformance.
