# Provider-side placement sizes (`placements.rs`)

`StorePlacements` is the `Placements` port a provider installs on its own `Client`, because the client re-prices every quote on both
sides and recovery, renewal and check prices depend on the payload size of the placement.

- Input: the operation key (whose requester must be the placement's sender or recipient), the provider and the placement ID.
- Output: the payload size recorded in the provider's own store.
- Failure: `Denied` for an unknown placement or a requester that is neither its sender nor its recipient, so sizes are never
  disclosed to third parties.

A requester installs its own catalog instead (the size it stored or discovered); a provider can never choose the size it is paid for.
