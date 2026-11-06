# Schema format validation

validate accepts an immutable JSON schema description. It requires an object and
nonempty type string. Primitive bool, signed/unsigned 32/64-bit integers, float,
double, string and bytes are Known. Novel types are Unknown, not silently mapped
to a familiar type. Malformed descriptors return Invalid. Unknown optional
metadata remains in the caller-owned document. This operation validates formats,
not invocation values, detached bodies, runtime classes or float encodings.
