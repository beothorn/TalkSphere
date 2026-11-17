# Authorized paid single-byte evidence

`Admission.new` configures a positive finite lease-history bound and `Policy`.
Both optional rules default to false: reject overlap while an earlier evidence
pin remains alive, and enforce a one-second minimum between accepted checks.
The interval is fixed at 1000 milliseconds; the API cannot configure an
undisclosed stricter frequency. Choose a tracking bound covering the finite owner
lease-history capacity. This state is process-local; reconstructing it after
restart can permit more checks, never a silently stricter rate limit.

`accept` receives `Context`, public challenge and value budgets. Context contains
original explicitly accepted paid Services check invocation, trusted owner image,
protected bytes, coherent retention observation, optional trusted reboot elapsed
seconds and an independent provider monotonic millisecond stamp. Callers must
already have check consent, payment/resource reservations and recovered owner
admission. The context is local authority, never peer-supplied identity or timing.
It validates definition/roles/current provided terms, exact positive checkPrice,
issuer and response interval, active lease ownership and zero-based offset.
Private expected-byte fields are rejected. Missing/foreign leases share Denied.
There is no included store allowance or inferred free check.

Successful acceptance retains `Pinned`, holding one immutable object range and
a local overlap marker. Repeated read access does not reauthorize or create a new
peer attempt. Retain the pin through provider evidence production; Services then
retains the complete original evidence independently. New inactive operations
fail while existing pins survive expiry/removal and defer physical reclamation.

`Pinned.evidence` reads the exact accepted byte and returns only leaseId, offset
and one-byte value. It supplies no outcome, expected value, price or full object
content. A wrong stored byte is still byte evidence for requester comparison;
this operation does not pretend that a one-byte check proves the whole-object
hash. Missing/short backing data returns Storage without successful evidence.

Optional overlap refusal ends when its last pin drops. With interval enforcement,
999 milliseconds is refused and 1000 is accepted. Backward or mismatched-epoch
stamps are invalid local clock input; the caller must recover/recreate admission
for a new clock instance instead of inventing elapsed time. Invalid or refused
attempts do not update the last accepted stamp. Capacity and arithmetic errors
remain Limit; inactive/refused overlap/interval returns Unavailable. This helper
neither settles checks nor decides requester correctness, refunds or trust.

From the repository root run
`cargo test --locked modules::storage::byte_check`.
Complete handler/network/accounting composition remains a later step. Behavior
follows pinned [Storage section 10](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#10-storage-check).
