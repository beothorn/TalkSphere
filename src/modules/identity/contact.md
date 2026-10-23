# Contact QR payload text

format receives a Contact with an expected binary ID and optional IP/port endpoint.
It returns canonical identity text alone, or that text followed by address and
port query fields. Address comes first in local output; IPv6 colons are encoded
as %3A, with no brackets or zone. Port is decimal without leading zeros.
Port zero and IPv6 scope/flow metadata return InvalidInput because the format
cannot carry them. The result is text for a QR renderer, not an image, credential,
network connection or trust decision. No private values are consulted.
