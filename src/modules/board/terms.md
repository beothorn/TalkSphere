# Economic terms and optional metadata

validate requires exactly one price/pricing representation, positive uint64
mathematical numeric price/checkPrice and a nonempty pricing.model in an object. positive
returns the exact positive amount or Invalid; no calculation uses floating point.
Embedded input/output/check schemas are forbidden. info must be an object.
Suggestions are inert objects with nonempty name and optional string version,
uri and description; empty optional strings are valid. Preconditions must be
an array of nonempty strings. Unknown metadata and models are preserved.

Upstream issue #9 tracks namespace validation: no precise grammar is specified.
This validator rejects empty names; unknown names remain unusable through the
semantic assessment port. It does not invent a namespace spelling requirement.
No hints, descriptions or URIs trigger external action or authorization.

Exact integral decimal/exponent spellings are evaluated with checked digit arithmetic,
never floating point; original spelling remains unchanged in forwarded bytes.
