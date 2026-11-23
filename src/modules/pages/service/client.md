# Page client

The requester contract for both roles. `Client::new(definition, catalogue, values)` accepts only the pinned definition. The price is the offering's fixed price (`terms` validates the offering). `validate_result`
requires the result to belong to its request (same path, size within the visitor's `maximumBytes` and the offering's limit) and to pass the local check at the accepted fixed price, so a lying hash, size, path or price
never authorizes settlement. `verify` is the local check: pass, fail or inconclusive computed from the request, the requester-owned expectation `{agreedPrice}` and the result; a missing expectation is inconclusive. The
client declares no remote check rules.
