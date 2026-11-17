# Store assessment before upload execution

store consumes an Input description, current compatible Offering, independent
Budget and authorized maximum service-work price. It validates exact positive
size/duration, current limits, required store fields, forbidden follow-up fields,
32-byte hash shape and data length equal to declared size. Complete bytes and
explicit unresolved binary references are both supported; text references are
not byte values. Duplicate/unknown extension content is still covered by the
original structural/byte budget. Unknown extensions remain semantically inert.

The returned Store contains a Services Assessment and exact integer work price.
Its local metadata projection is operation/store, sizeBytes and retentionSeconds;
provider-only factors are empty. The full declared input byte bound includes
hash, data and field names/extensions. It creates no lease, execution, financial
reservation or storage file, and returns no provisional owner pin. Provider
composition must apply recovered capacity policy before receiving/allocating work.

requester_metadata derives the same projection from authorized complete local
input without cloning payload bytes. store_price recalculates the exact fixed
work price from this validated local metadata and empty factors. It does not put
requester-known dimensions into peer pricingInputs or invent a wire field.

Assessment validates unresolved hash shape without inventing its bytes. Actual
hash/data consistency must be checked after complete input and before durable
lease acceptance; valid small detached hash bodies remain supported. Assessment
success does not authorize an incomplete body or establish hash validity.

Malformed fields/types/lengths are Invalid; configured/advertised bounds are
Limit; work price exceeding maximum is Insufficient. Services/composition owns
authentication, separate consent, total spending bounds and issuer fees. This
helper is not a registered standard provider or authenticated network service.

On Linux, admitted_store performs the same assessment through a recovered Gate
and probes fresh-key byte/lease/pending-work capacity before upload admission.
It rejects existing keys and exhausted capacity without changing any hold. No pin
is retained: execute must reserve/recheck before storage writes, and later failure
is explicitly uncharged when uncommitted. This does not promise disk availability
or replace finite Transport/Services body buffering limits.
