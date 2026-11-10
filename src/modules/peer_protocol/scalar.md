# Exact numeric control fields

uint64 accepts only canonical unsigned decimal strings, with zero represented as
0 and all positive values without signs or leading zeros; overflow above uint64
is Invalid. uint32 accepts an exact JSON integer number token without fraction,
exponent or negative sign and checks the declared width. Neither passes through
binary64. These helpers perform representation validation, not price arithmetic
or economic authorization. Subsequent service scalar binding extends this file.

Decode takes a schema type, JSON value and trusted nonfinite permission. Bool and
string keep their logical kinds; int32/uint32 use integer tokens, int64/uint64
canonical decimal strings. Float/double require exact single-member lowercase
IEEE bit objects with8/16 hex digits. Signed zero is preserved. Forbidden NaN or
infinity is Invalid; permitted NaN requires7fc00000/7ff8000000000000. Unsupported
types fail explicitly rather than coercing. Encode takes a typed Services Value
and the same permission, preserves exact integers/finite bits and emits canonical
NaNs. Bytes, detached values and containers are separate layers. Callers must
obtain nonfinite permission from installed contract semantics, never peer input.
