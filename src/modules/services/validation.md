# Logical schema validation

validate takes an immutable logical Value, schema descriptor, finite Budget and
trusted permission for nonfinite floating-point values. It checks schema support,
then aggregate value budgets, then exact types and required nested fields.
Missing optional fields are valid; values cannot represent null. Unknown object
fields remain inert but consume budgets. Integers never convert widths or pass
through floating point. Nonfinite permission must come from understood registered
contract semantics, never directly from arbitrary peer metadata.

Invalid denotes malformed schema or mismatching value; Unsupported means an
unknown type; Limit means schema or value budgets exhausted. Validation has no
execution, spending or storage effects. Registered family validation must also
check discriminator restrictions and content-specific invariants; passing this
shape validator alone does not authorize an invocation.
