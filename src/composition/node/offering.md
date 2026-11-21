# Storage offering

`prepare(local, terms, catalogue)` builds the one provided Storage offering
(`storage-standard`, service `cr2se.storage` version 1, issuer = the local identity)
from the configured pricing, check price and maxima. The Board parser and the Storage
standards catalogue validate it, so unrepresentable prices, zero denominators or
inconsistent maxima are refused before anything is published; error messages name the
failing stage, not the values. The result holds the Board document, the complete
standard definition and the definition document bytes.

With no terms (a requester-only node) the Board document is empty and nothing is
advertised, but the default definition is still produced because the requester client
needs it to validate quotes and results.
