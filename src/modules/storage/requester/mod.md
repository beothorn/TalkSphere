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
