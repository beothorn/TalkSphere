# Page Peer Protocol intents

`Remote::new(terms, selection, cache)` holds one publisher's validated Page offering and builds the endpoint plan for a `get` with an exact spending ceiling equal to the offering's fixed price, so a quote above it is
refused before anything is reserved. `get(request)` is `Invalid` for a path above `maximumPathBytes` or a `maximumBytes` of zero or above `maximumResourceBytes`. Each further resource is a separate plan at its own price;
nothing a page returns can authorize more spending.
