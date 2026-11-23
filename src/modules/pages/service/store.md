# Page resource map

`Pages` is the port the provider reads: `snapshot(path)` returns one immutable snapshot (media type and shared bytes) of the current resource or none, and `check_root(terms)` requires the root `/` to exist as
`text/html` within the offering's resource limit (the condition for serving or advertising an offering). Snapshots are reference counted, so replacing or removing a resource never changes a response already pinned.
`MemoryPages` is replaced and removed by local tools only (no peer can publish, edit or list). `DirectoryPages(root, maximumBytes)` maps `/` to `index.html` and `/a/b` to the file `a/b` under the root, labels by
extension (unknown is `application/octet-stream`), refuses symbolic links at every component and anything outside the root, and reports a file above the bound as `Limit`.
