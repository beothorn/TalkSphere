# Logical values

Value represents bool, exact int32/int64/uint32/uint64, binary32/binary64 bits,
UTF-8 strings, arbitrary bytes, ordered arrays and sequences of named object
fields. It has no null variant: omission is expressed by a missing object field.
Raw float bits preserve signed zero and NaN payloads. Schema validation must
separately reject nonfinite values unless the understood contract permits them.
No implicit numeric conversion or normalization occurs.

Budget bounds nesting depth (root zero, with an independent hard ceiling of31), aggregate nodes and aggregate text,
byte payload and field-name bytes. Value.bounded visits the caller-owned value,
returns Limit on exhaustion and Invalid on empty/duplicate object names.
Ignored extension fields still consume budget. This is a local representation,
not a peer encoding; callers must bound decoding before constructing huge values.

byte_len applies the same bounded traversal and returns aggregate payload/name
bytes for caller byte-grant checks. It does not count native object overhead;
node admission separately bounds container overhead.

`body_count` validates the same budgets and counts each logical byte value, including empty values. Containers and text do not add body references.
