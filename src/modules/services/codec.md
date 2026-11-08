# Bounded local value codec

encode and decode preserve exact logical Value types under Budget and maximum
encoded bytes. This is a private/local binding, not CR2SE peer JSON or body records.
Tags identify bool, exact integer widths, float bits, text, bytes, objects and
arrays; lengths are uint32 big-endian. All decoded lengths are checked against
available bytes and aggregate admission before copying. Recursion is capped at31.
Unknown tags are Unsupported; malformed/truncated/trailing data, invalid UTF-8,
booleans or object names are Invalid; exhausted budgets return Limit. Encoding
never returns a partial result. Ownership transfers only after complete validation.
The coordinator image supplies its own version; adapters must also reject unknown
local call-contract versions before decoding or dispatching effects.
