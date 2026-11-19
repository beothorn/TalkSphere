# Installed paid byte-check rules

Rules declares Paid for a valid current provided Storage offering and public
challenge. There is no included store allowance. Response seconds convert to
milliseconds with checked arithmetic; unsupported local representation fails
Limit. Shape checks use independent installed schema and Storage restrictions.

Challenge authorization checks original accepted public check terms, exact
positive price/response budget/issuer, original requester/provider/key and the
independent provider-scoped lease snapshot. Known inactive leases and invalid
offsets fail. Actual provider execution must independently establish active
retention and pin accepted bytes; requester metadata never overrides provider
clock/state admission.

Evidence schema has only leaseId, offset and one-byte value. Complete evidence
must match the original public challenge; a provider outcome or malformed value
cannot authorize settlement. This check establishes shape/association rather
than correctness. Private original bytes are never read from evidence fields.

Outcome consumes trusted original Services Timing, complete evidence, and a
requester-only expectation object with leaseId, offset, expectedByte (one byte)
and active:true. These fields are local observations, never challenge/wire
pricing fields. The application must construct them from its own data and
reliable original activity knowledge; inactive/ambiguous activity must not be
asserted as active. Services computes Within only from comparable original
send/complete-response observations inside the accepted bound. The family never
samples a fresh worker clock or trusts provider timestamps.

Within plus exact private expected bytes passes; timely wrong bytes fail.
Missing/malformed private data, ordinary provider evidence supplied as an
expectation, missing evidence or ambiguous/missing timing is inconclusive.
Explicit authenticated unavailability classification remains available through
the separate typed requester byte verifier; generic failures are not converted
into storage-loss proof here. No outcome changes Ledger, provider state, refunds
or local trust. Requester and provider accounting observations stay separate
under [CR2SE #10](https://github.com/beothorn/CR2SE/issues/10).
