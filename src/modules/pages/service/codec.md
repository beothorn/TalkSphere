# Page codec

`GetRequest {path, maximumBytes}` (no `operation` field): `path` must be a canonical path and `maximumBytes` an exact uint64. `GetOutput {path, mediaType, contentSize, contentHash, content, price}`: a canonical path
equal to the requested one, a valid media type, the exact size, the 32-byte SHA-256 and the complete content, whose length must equal `contentSize`. Duplicate names and wrong types are `Invalid`; unknown fields are
ignored (compatible extension).
