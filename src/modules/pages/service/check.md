# Page local check

The visitor judges one successful `get` itself, sending nothing to the publisher and paying nothing. `Pass`: the returned path equals the requested path, the media type is valid, `contentSize` equals the content length,
`contentHash` equals SHA-256 of the content and the returned price equals the agreed price. `Fail`: every needed value is present but at least one condition is false (an invalid label, a hash of the wrong length and a
lying size all fail). `Inconclusive`: the request, the agreed price or the output is missing, or the content exceeds the local hashing budget. A pass proves only self-consistency; it is not a resource signature or a
transferable proof, and a publisher's claim that a result passed is never trusted.
