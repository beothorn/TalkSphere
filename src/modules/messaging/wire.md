# Wire

Messaging requester intents for the Peer Protocol endpoint. The endpoint already validates quotes, reserves, commits once and receives results only on the original stream; this file adds the typed intent with an exact spending bound.

- `Remote(terms, selection, cache)`: one provider's Messaging offering, selected from its directly retrieved metadata.
- `store(input)`, `discover(input)`, `recover(input, payload_size)`, `status(input)`, `renew(input, payload_size)`, `remove(input)`: each returns the endpoint plan and the exact price the requester itself calculated from the offering (and, for recoveries and renewals, from the payload size it knows). That price is both the spending maximum and the consent ceiling, so a provider quote above it is refused before anything is reserved. A store outside the offering's size or retention limits is `Invalid`.
- `check(input)`: a paid byte challenge at the fixed check price; the requester later judges the evidence with its private expectation.
- `Remote::retry(original)`: retransmits an original recovery or removal over a new stream through the endpoint's cached-retry path, with no new quote or charge.
