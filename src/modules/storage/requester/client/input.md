# Installed Storage value semantics

Client implements Services Semantics for its own independently installed input,
output, check input and logical check-output descriptors. Another descriptor is
Unsupported. Ordinary operation restrictions reuse Storage's standard validator;
output discrimination does not treat output as a full input. Nonfinite numerical
semantics are not enabled.

Store quote metadata contains only operation, sizeBytes and retentionSeconds;
complete original data must have exactly the declared positive byte length, and
hash width and operation field restrictions remain checked. Current limits and
prices are checked separately by scoped quote callbacks. Complete input remains
available to normal Services body ownership and later hash/result validation;
no unresolved body is replaced by fabricated bytes.

Follow-up metadata retains bounded complete original input. Counts, prices,
offsets and times preserve exact uint64 values; logical budget failures precede
copying or semantic inspection. Caller and installed policy bounds both apply.

Check input contains only public challenge information under compatible extension
rules. Expected bytes and provider correctness claims are rejected. Logical check
output has requester-owned pass/fail/inconclusive, a fixed lease ID, uint64 offset
and optional exactly one-byte value. Provider evidence uses separate Rules and
cannot supply this logical outcome.
