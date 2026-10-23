# Contact QR payload text

format receives a Contact with an expected binary ID and optional IP/port endpoint.
It returns canonical identity text alone, or that text followed by address and
port query fields. Address comes first in local output; IPv6 colons are encoded
as %3A, with no brackets or zone. Port is decimal without leading zeros.
Port zero and IPv6 scope/flow metadata return InvalidInput because the format
cannot carry them. The result is text for a QR renderer, not an image, credential,
network connection or trust decision. No private values are consulted.

parse(input, maximum_bytes) accepts an ID alone or one address and one port query
field in either order. It checks the byte limit before decoding and preserves the
expected binary ID. Percent escapes are decoded once, reserved raw address bytes
are rejected and the resulting value must be an IP literal. Port is ASCII decimal
1..65535 without signs or leading zeros. Unknown/duplicate/missing fields, fragments,
DNS, brackets, zones, invalid UTF-8, malformed escapes and double escaping fail with
InvalidInput. Excessive input returns ResourceLimit. Results remain unauthenticated
contact hints; parsing has no socket, DNS, trust or Discovery effects.
