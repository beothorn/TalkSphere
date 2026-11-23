# Hosted publication Peer Protocol intents

`Remote::new(terms, selection, cache)` holds one host's validated offering and its direct-metadata selection, and builds the endpoint plan for each operation with an exact spending ceiling equal to the price the
requester itself calculates, so a quote above it is refused before anything is reserved. `store` (exact price for the publication's size and retention; `Invalid` outside the offering), `list` (fixed list price), `get(request,
expectedSize)` (the exact get price for the size the reader learned from a list entry; `Invalid` for zero or a size above `maximumBytes`, so a host that quotes a larger size is refused), `renew(request, contentSize)`,
`remove` (fixed price; the quoted release credits must meet the minimum) and `check` (fixed check price). `retry(originalKey)` retransmits a removal over a new stream without a new quote or charge.
