# Provider-side placement sizes

`StorePlacements(store)` is the `Placements` port a host installs on its own `Client`, because the client re-prices every quote on both sides and a renewal's price depends on the content size of the placement.
Input: the operation key (whose requester must be the placement's publisher), the provider and the placement ID. Output: the content size recorded in the host's own store. `Denied` for an unknown placement or a requester that
is not its publisher, so sizes are never disclosed to others. A requester installs its own catalog instead (the size it stored); a host can never choose the size it is paid for.
