# Participant journal

One durable entry per Services operation key: `Prepared` (the exact effect and the 32-byte token that authorizes its commit), `Committed` (the outcome, retained so a lost reply is answered from the original result) or `Aborted`. Native entries are `<requester><connection><stream>.json`
files (0600, in a 0700 directory) replaced atomically; opening refuses a corrupt, oversized (over 512 KiB), misnamed or unknown-state entry without repairing it and discards an interrupted temporary file. The journal is bounded: a new key beyond the bound evicts the oldest finished entry, prepared
entries are never evicted and a journal full of them returns `Limit`. `get`, `put`, `prepared` (for recovery); the volatile form can inject write failures.
