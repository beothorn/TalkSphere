# Framing contract

WireFrame retains a validated header, matching payload and optional 16-byte tag.
Its constructor rejects a mismatched length. Header, payload and tag accessors
permit verification; into_payload transfers ownership without proving validity.
Fields are private and output is not automatically safe for application dispatch.

FramingError distinguishes Header errors, Truncated input, socket Io errors,
Closed state, a PhaseChanged caller error, local Shape mismatch and writer Busy.
These are local outcomes, not wire codes. The future connection driver maps them
to safe closure/error responses. FrameReader and FrameWriter are re-exported.
