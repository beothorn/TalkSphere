# Inline byte values

INLINE_MAX is4096 decoded bytes. Encode produces the single-member $base64
wrapper with canonical padded Base64 and rejects larger input with Limit so the
caller must use a detached body. Empty bytes are always inline. Decode requires
exactly that one string member, rejects unknown wrapper siblings, checks decoded
size before allocation and preserves arbitrary bytes. Invalid spelling or kind
is Invalid; oversized decoded input is Limit. The shared Base64 helper has no
service-size policy; this layer enforces the wire threshold independently of
fixed identifiers and binary body chunking. Signed/canonical family objects must
be decoded to exact bytes before hashing or verifying their signatures.
