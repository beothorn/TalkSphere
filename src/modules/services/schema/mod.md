# Schema format validation

validate accepts an immutable JSON schema description. It requires an object and
nonempty type string. Primitive bool, signed/unsigned 32/64-bit integers, float,
double, string and bytes are Known. Novel types are Unknown, not silently mapped
to a familiar type. Malformed descriptors return Invalid. Unknown optional
metadata remains in the caller-owned document. This operation validates formats,
not invocation values, detached bodies, runtime classes or float encodings.

Composite validation recursively checks object.fields and array.items. Named
fields require nonempty semantic descriptions. optional must be a boolean and
is legal only on object fields; required is the default. Unknown nested types
propagate Unknown to the complete schema. Recursion beyond 31 returns Limit.
check validates a nonempty procedure description and complete input/output
schemas; malformed descriptions return Invalid. Neither function accepts values
or invokes the described check procedure.

The independent schema Error contract distinguishes Invalid and Limit without
importing Board types. A single validation is bounded to 65536 schema nodes and
depth 31 even outside Board; consuming modules map these failures at their boundary.
