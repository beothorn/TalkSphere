# Requester retrieval verification

`validate` accepts the original authenticated accepted invocation, complete
original request, complete successful output, independently retained lease,
optional private original range bytes, and logical value budgets. Callers admit
successful Services completion before calling and keep partial stream fragments
provisional. The output is borrowed, so its owner must outlive the result.

It checks Storage version/definition, provided offering and original roles,
lease ID, subtraction-safe range bounds, current accepted retrieval limits,
empty factors, exact price, original offset, exact byte count and independently
retained whole-object hash. Any mismatch fails without returning usable data.
Later expiry or removal does not invalidate an originally timely accepted read.
It does not establish that the operation was accepted timely; trusted Services
and provider admission supply that fact.

`Retrieved.data` borrows the complete accepted range. Its two booleans describe
separate checks: `whole_object_hash_checked` is true only for a full-object read
whose SHA-256 matches the independent lease; `requester_bytes_compared` is true
only when supplied original range bytes match exactly. A supplied expectation
of a different length or content fails. A partial read without an expectation
checks metadata only and makes no cryptographic claim about those range bytes.
The returned whole-object hash is never substituted with a hash of a partial range.

Invalid covers malformed values, ranges, metadata, data and price disagreement;
Denied covers inconsistent ownership/roles; Unsupported identifies another
contract, and Limit preserves exhausted budgets/arithmetic limits. No provider
work, mutation, payment, remote retry or provisional application output occurs.

Behavior follows [Storage section 7](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#7-retrieve).
