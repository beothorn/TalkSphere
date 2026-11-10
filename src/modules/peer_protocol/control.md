# Lossless control documents

Control.parse takes exact source bytes and a positive aggregate node budget. It
requires1..1048576 UTF-8 bytes, one complete JSON object and a nonempty string
type. Preflight rejects duplicate decoded names at every depth, malformed/trailing
syntax, BOM, and more than32 nested containers before building the tree. Numeric
values use arbitrary precision and never pass through binary64. Nodes exhaust
with Limit; malformed input is Invalid. Unknown/null optional fields remain inert;
message-specific required/null/type rules belong to the message validator.

Bytes returns the immutable original JSON body for exact quote hashing. Value
borrows the complete lossless tree; neither getter permits mutation. No raw input
or per-byte diagnostic event is logged. This parser is wire-specific; it does not
relax Board's document contracts or imply exchange authority.
