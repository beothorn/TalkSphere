# Requester lease catalog

The catalog is the requester application's own record of leases bought from providers.
It stores, per lease: the opaque ID, provider identity and last contact, the accepted
terms, size, content hash, acceptance and expiry times, store price and state. The
file `catalog.json` is owner-only, replaced atomically (write a private temporary file,
flush, rename), and holds no encryption keys. Providers never receive it.

- `open` loads an existing file (absent means empty). Oversized, malformed, duplicate,
  wrong-owner or wrongly typed content is refused as a whole.
- `create` writes an empty catalog and refuses an existing file.
- `put` retains or replaces a validated lease together with its provider contact; a lease
  for another requester is refused. A failed write leaves memory and the file unchanged.
- `get`, `find` and `list` read entries; the same lease ID at two providers stays separate.
- As the Storage requester port, a lookup succeeds only for the owning requester and the
  exact provider and ID; every other lookup reports an access denial.
