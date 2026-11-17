# Storage requester contracts

[Store result validation](store.md) compares complete output with original
requester input and accepted invocation terms. [Catalog retention](catalog.md)
uses only acknowledged results observed through public Services contracts.

These operations do not execute provider work, create remote retries or select a
new acceptance-time rule. Provider lease creation still awaits CR2SE issue #11.
