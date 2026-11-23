# Hosted page manifest

A hosted page is a `page` publication whose content is the manifest; each resource is its own `resource` publication by the same publisher with the same visibility and audience. The canonical manifest bytes are
ASCII `CR2SE-HOSTED-PAGE-MANIFEST-V1`, a 4-byte big-endian resource count, then for each resource ordered by path bytes ascending without duplicates: a length-prefixed path, a length-prefixed media type (each
`u64be(length)` then the UTF-8 bytes), the content size (8 bytes), the SHA-256 of the content and the resource publication ID. The count is at least one and at most the offering's `maximumManifestResources`; the root
path `/` must be present with media type `text/html`; paths are at most 1024 bytes and media types at most 127.

- `Manifest::new(entries, maximumResources)` sorts the entries and enforces the rules (`Invalid` for none, too many, a duplicate path, an over-long path, a missing root or a root that is not `text/html`).
- `encode()` gives the canonical bytes; `decode(bytes, maximumResources)` is strict and bounded: wrong domain, zero or excessive count, a count the bytes could not hold, truncation, an oversized length, an invalid
  path or media type, unsorted or duplicate entries, a bad root and trailing bytes are all `Invalid`, and lengths are compared with the remaining bytes before anything is taken.
- `entries()` lists in canonical order; `entry(path)` finds one.
The independent vector V2 (a single root entry) is reproduced byte for byte.
