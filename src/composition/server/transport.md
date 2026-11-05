# Real Transport composition

compose takes validated Config and loaded Credentials by ownership. It constructs
TcpProvider and a SystemClock sharing that provider's readiness worker, then a
SystemEntropy-backed Cryptography adapter and Driver with the scoped Identity.
No listener or outgoing socket is opened by construction. Failure returns Runtime
and drops acquired credentials/dependencies; no test substitutes or fallback keys
are selectable. The cryptographic input budget supports full frame AAD.

Transport owns the RealDriver, retained configuration, public local ID and copied
contacts. Trusted application composition receives local control authority through
the driver; this object is never reconstructed from a peer message. Dropping it
releases driver resources and ultimately the protected credential-store lock.

listen is valid once in Starting. It submits the validated endpoint to Driver,
retains the listener capability and transitions to Ready only on successful bind.
Its Ready result contains the actual bound endpoint (including an assigned port)
and public local identity. Bind/admission failure is Network; repeated or late calls
are Runtime. Dropping Transport releases the listener and permits binding again.
