# Requester removal validation

`validate` takes original authenticated accepted invocation, complete original
input, complete acknowledged successful output, independently retained lease
metadata and bounded value policy. Callers admit complete Services success before
using this function; provisional fields cannot update an application catalog.

It checks contract/schema, provider/requester identities, current provided
Storage offering, empty factors, original lease ID and exact accepted removePrice.
Successful output must say remove, have the same lease ID and price, and contain
removed:true. Success returns an owned copy with only state set to Removed.
Original store price, size/hash, identities, acceptance and expiration and accepted
terms remain unchanged. The caller owns later catalog publication.

There is no provider work, financial change, refund, physical-erasure promise or
remote replay operation. Validating original acknowledged success again is not
permission to invoke remove again or charge it twice. Denied rejects inconsistent
roles; Invalid rejects malformed output, IDs, factors and prices; Unsupported
identifies another contract; Limit preserves bounded value/arithmetic failures.

Run `cargo test --locked modules::storage::requester::remove` from the repository
root. This follows pinned [Storage section 9](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#9-remove).
