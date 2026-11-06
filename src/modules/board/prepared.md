# Complete publication candidate

Prepared.new accepts publisher, complete Board bytes, definitions indexed by ID,
Limits and trusted Capabilities. It validates aggregate sizes before decoding,
requires exactly one matching definition per offering, rejects extra definitions,
checks complete response retrievability and conservative same-service/version
contract consistency, then requires local semantic eligibility. Failure has no
publication, filesystem, registration or execution effect.

Candidate state is private and immutable. publisher and bytes expose the bound
identity and exact aggregate source size. assess rechecks current trusted semantic
capabilities before commit, including provider versus wanted requester support.
Unknown models/types remain discoverable in parsed documents but cannot become
local advertised capabilities through this constructor. Peer JSON gains no local
snapshot revision field. Atomic visibility is implemented separately by the owner.
