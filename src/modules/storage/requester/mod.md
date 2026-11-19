# Storage requester contracts

[Store result validation](store.md) compares complete output with original
requester input and accepted invocation terms. [Catalog retention](catalog.md)
uses only acknowledged results observed through public Services contracts.

These operations do not execute provider work, create remote retries or select a
new acceptance-time rule. Provider lease creation still awaits CR2SE issue #11.

[Retrieval verification](retrieve.md) compares complete results with independent
lease metadata, hashes full objects, and reports partial-range integrity limits.

[Renewal validation](renew.md) checks original guards, prices and previous/new
expiration before returning updated independently retained lease metadata.

[Removal validation](remove.md) returns terminal metadata only after matching
original accepted identity, price and true removal success.

[Private byte-check verification](byte_check.md) consumes original authenticated
transport observations and independent expected bytes without changing accounting.
[Secure sampling](sampling.md) optionally chooses unbiased private offsets.

The [installed Client](client/mod.md) implements actual Services requester and
paid-check ports using these validators and independently retained original
provider/attempt expectations. It enables no provider registration by itself.
