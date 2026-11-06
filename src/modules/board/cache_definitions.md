# Cached definition association

Cache.attach takes trusted Cache scope, a selected fresh Remote receipt, exact
offering ID, complete definition bytes and injected current time. It validates
all identity/schema fields and replaces the cache entry with a new immutable
receipt containing that association. Aggregate bytes and retained receipts remain
bounded. Missing IDs, mismatches and stale/delayed responses fail; no nearby
version or automatic refetch is substituted.

A receipt must still be the current exact revision. Each successful attachment
returns the new receipt for subsequent attachments; older pins retain their
original bytes and definitions but cannot acquire delayed definitions. definition
requires Read authority and a current/fresh receipt, returning exactly the requested
validated association or Missing. Unknown schema types remain explicitly Unknown.
