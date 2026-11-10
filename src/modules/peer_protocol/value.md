# Schema-bound values and provisional metadata

Encode takes a typed Services Value, finite Limits and trusted nonfinite
permission. It checks logical depth/node/byte budgets, sorts object fields by
unsigned UTF-8 bytes and visits arrays by index. Encoded owns metadata plus
immutable Blob bodies indexed from zero. Bytes above4096 and text above4096 UTF-8
bytes detach; empty values stay inline. Small nonempty detached values remain
valid to receive. The enclosing control encoder separately checks JSON size/depth.
Callers validate their installed family schema before proposing or returning work.

Descriptor contains sequential uint32 id, positive exact uint64 length and a text
flag. Pending.parse takes received metadata, retrieved known schema, Limits and
trusted nonfinite permission. It independently checks required/optional fields,
scalar types, exact descriptor members, IDs/traversal and declared byte/body
capacity before body allocation. Descriptors exposes immutable requirements;
metadata alone provides no complete application value or execution permission.
Unknown optional fields are ignored as application values, while reserved body
descriptors in them remain bounded transport obligations and consume IDs in the
same traversal. They cannot add application behavior or financial authority.

Assemble requires exactly every declared body with exact length and valid final
UTF-8 where text, then materializes only known schema values under the logical
budgets. No partial value is returned. Invalid wrappers, duplicate/out-of-order
IDs, missing/extra data, null in a known nonnullable position and unsupported
schemas fail explicitly; exhausted capacities are Limit. Container tree walks
emit no per-node payload logs; public operations mark bounded control flow.
Actual record order, input/result terminal markers, normal END and deadline/
authority are additional exchange responsibilities, not proven by assembly.
