# Recovery before new-work admission

Gate.new binds a durable Journal, protected Linux byte Store and finite resource
policy. It starts closed. Privileged completion/status/prepare/commit/abort ports
remain usable for original coordinator recovery while new work is unavailable.

Resume supplies the authoritative coordinator identity and resumes an original
operation. Services is the direct adapter to public Coordinator.resume with its
normal authorized Context; it reads no private coordinator schema. Missing or
uncertain decisions propagate. Storage never converts them into abort.

recover serializes recovery and closes admission before reload. It asks the
matching coordinator to resume every retained original operation, including
family-terminal records whose remaining participants may still be incomplete,
then requires matching terminal family status. It advances the retention anchor only after all such
records resolve, persists terminal expirations, verifies active byte size/hash,
and inventories every byte entry. Receiving or unreferenced entries keep admission
closed until explicitly resolved by the owner; they are not silently deleted.

Quota reconstruction retains original operation keys and accounts for complete
result encodings/buffers, lease history and all proposed create bytes. Until
explicit reclamation is implemented, terminal/orphan bytes remain conservatively
charged. Committed lease metadata must match unique original create records.
Insufficient configured limits do not evict existing obligations. Reboot progress
requires independently trusted elapsed time; clock uncertainty remains unavailable.

Only after every check succeeds does is_ready report true. admit runs one trusted
bounded local action under the admission lock, supplying the journal and quota.
The action must reserve before writes, retain uncertain holds and avoid gate
reentry/network waits. Unknown closes admission. Lifecycle decisions adjust known
pending-work usage after acknowledgement; any ambiguous or inconsistent update
closes the gate and preserves recovery requirements.

No function executes service work, creates a new peer invocation or infers remote
receipt/payment from local commitment. Local caller authority and retained handler
registration remain composition responsibilities. Paid Storage handlers and full
node startup composition are later steps.

This finite composition retains matching coordinator history for retained family
records. It does not support coordinator-history compaction without an additional
verified local finalization contract; missing history closes admission rather
than treating a family commit as proof of complete local coordination. Current
Services retains its original operation records. This is a local profile limit,
not a new peer requirement or a protocol defect.
