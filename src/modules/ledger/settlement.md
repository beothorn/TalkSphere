# Ledger participant preparation

Prepare requires Coordinator authority, a local Reservation, final positive price
and a nonempty bounded complete result/effect reference. Services is responsible
for validating and durably retaining the result behind that reference. Fixed price
must equal the accepted maximum; metered price is at most maximum. Exact retries
return the same owner/key/effect token; changing any price/reference fails Conflict.
The effect and Prepared status are durable before success. Prepared resources
remain held after deadlines, cancellation or a lost reply.

Prepared is opaque and binds local identity, attempt key and a domain-separated
hash of final price and exact reference bytes. It is not authority or a peer field.
Status requires Coordinator authority and reports absent or Reserved/Prepared/
Committed/Aborted from the recovered image; it never repeats service execution.
Only later explicit commit/abort decisions change a prepared effect.
