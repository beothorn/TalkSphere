# Complete body ownership

Body.new admits exact declared uint64 length against address space and configured
maximum before allocation. An optional expected SHA256 is the contract's agreed
content hash. push accepts only the next exact offset and bytes within the declared
length. Gaps, overlaps or excess data poison the body permanently. Fragments stay
provisional and produce no Blob. No per-fragment entry event is logged.

finish consumes the body, requires exact completion and a matching agreed hash,
then transfers bounded immutable bytes into Blob ownership. Missing, extra or
corrupt content is Invalid. Body does not charge, execute work or interpret the
family's logical schema; requester validation independently checks complete input
against accepted metadata. The peer adapter owns record order and explicit end/error.
