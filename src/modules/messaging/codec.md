# Messaging codec

Between the typed operations and the Services logical `Value` model. Wire encoding belongs to Peer Protocol.

- `parse_request(value) -> Request` and `request_value(&Request)`: the six operations `store`, `discover`, `recover`, `status`, `renew`, `remove`. The
  `operation` member selects the operation; each operation's members are required with their exact logical type (`uint64` times, sizes and credits,
  `uint32` limit, `string` encoding, exactly 32 bytes for IDs, hashes and keys, exactly 64 for signatures). A known member that belongs only to another
  operation is `Invalid`; an unknown member is ignored (a compatible extension); duplicate names, a non-object, a missing or wrongly typed member and
  a wrong length are `Invalid`; an unknown operation or payload encoding is `Unsupported`.
- Store: `payloadSize` must be at least one and equal the payload length. Discover: `limit` at least one; `afterAcceptedAt` and `afterPlacementId` both or neither
  (the upper bound is checked against the offering when the request is assessed).
- `parse_response(value) -> Response` and `response_value(&Response)`: the same six, with the specified output members. Recovery output carries the original
  message and the receipt (no price member). Status is `pending` or `recovered`; a recovered status carries the whole receipt and a pending one none of it.
  A recovered payload must match its `payloadSize`.
- `check_input_value`, `parse_check_input`, `check_output_value`, `parse_check_output`: the paid check challenge and its evidence, which is exactly one byte.

Decoding trusts no peer size: lengths and counts are compared, never used to allocate before validation.
