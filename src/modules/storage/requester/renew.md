# Requester renewal validation

`validate` takes original authenticated accepted invocation, complete original
renewal input, complete acknowledged successful output, independently retained
original lease, and bounded value policy. It checks matching definition/schema,
provider/requester roles, the provided offering, original guard, exact sole
retained-size factor, current limits and price. Output must have the same lease
ID, exact previous expiry, checked new expiry, and accepted exact price.

On success it returns an owned lease copy with only expiration updated. The
caller retains that copy through its application catalog after normal successful
Services completion. It does not mutate the catalog, reserve credits, execute
provider work, or interpret provisional output as success. Later local state
changes cannot establish whether original provider acceptance was timely.
Revalidating against an already extended lease is a stale guard, not permission
for another charge or extension.

Denied rejects inconsistent roles; Invalid rejects malformed or inconsistent
output or economics; Conflict rejects stale expected expiry; Limit preserves
budget/arithmetic/current-limit errors; Unsupported identifies another contract.
Run `cargo test --locked modules::storage::requester::renew` from the repository
root. This follows the pinned [Storage renewal contract](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#8-renew).
