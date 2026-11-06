# Board resource policy

Limits contains positive budgets for document bytes, container depth, JSON node
count, decoded string bytes, array elements, definitions, aggregate publication
bytes, cache entries, retained revisions and cache lifetime in monotonic seconds.
Defaults are respectively 1048576, 31, 65536, 1048576, 4096, 1024, 16777216,
64, 128 and 300. These are local refusal policies, not new wire maxima.

Validation rejects zero budgets, depth above 31 (reserving the peer response
wrapper), document size above 1048576, strings larger than a document, and an
aggregate publication budget smaller than a document. Validation has no effects.
Actual wrapped response length and depth require additional checks before
publication; these defaults do not assert that a maximal standalone document fits.

Private record envelope arithmetic must also fit the platform integer width;
publication/count combinations that overflow are rejected before filesystem effects.
