# Metadata response bounds

encode takes kind board or definition and an immutable Document. It emits the
complete UTF-8 JSON control object with the original document embedded verbatim,
including unknown members, escaping and whitespace. Unsupported kinds fail.
The exact body must be at most CONTROL_BYTES (1048576); the five-byte record
header is outside this budget. Document has at most 31 containers and the response
adds one, preserving Peer Protocol's depth-32 bound. Excess length returns Limit
before allocating the response. This helper implements no network exchange.
