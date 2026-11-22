# Replica book

A durable, bounded record of hosted items in the replicas directory (`book.json` plus one `<id>.sealed` file per item, all
owner-only, the directory 0700).

- **Item**: random 16-byte ID, plaintext size, sealed size, SHA-256 of the plaintext, creation time and confirmed replicas.
- **Replica**: provider identity, lease ID, expiry, confirmation time and last conclusive check. Confirmed replicas of one item
  are at distinct providers; at most 64 per item; at most 1024 items.
- `add(item, sealed)`: writes the sealed bytes first, then records the item; a failed write leaves no item.
- `take_for(provider, target, maximum_bytes)`: reserves the item with the fewest confirmed plus reserved copies below `target`
  (oldest first) that the provider neither holds nor has reserved and whose sealed size fits; returns its ID and sealed bytes.
- `release(id, provider)`: gives a reservation back. `confirm(id, replica)`: records a validated replica and ends the
  reservation; refused for an unknown item, a repeated holder or the replica bound.
- `sealed(id)`, `get(id)`, `list()`: reads; a missing or wrong-sized sealed file is `Corrupt`, never returned.
- A corrupt, duplicated, oversized (over 4 MiB) or unknown-member book file is refused and left untouched.
