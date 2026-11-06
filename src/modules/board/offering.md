# Board and offering identities

Board.parse consumes bounded JSON and Limits, requiring version 1 and both
providedServices/wantedServices arrays. Empty arrays are valid. Unknown versions
return Unsupported; malformed fields return Invalid. Each offering requires
nonempty id, service and description, serviceVersion in 1..4294967295 and a
strict textual creditIssuer decoded through Identity. Aliases are invalid.

Offering preserves its complete source object and exact issuer bytes, plus
Provided/Wanted direction. This direction does not depend on TCP roles. Board
indexes offerings by unique ID across both lists and rejects duplicates rather
than selecting first/last. Definition count bounds total offerings. Array order
supplies no priority. Unknown service identifiers are metadata, not automatically
supported contracts. Exact economic and optional metadata validation is applied before returning the Board.

string reads a required nonempty string. integer reads an exact nonnegative mathematical numeric
value without coercing strings, booleans or floating-point values.
