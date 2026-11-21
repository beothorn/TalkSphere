# Node configuration

`node.json` is trusted operator input. It is a JSON object that must contain
`"version": 1`; every other member is optional and replaces only what it names.
Unknown members, non-integer numbers and zero limits are refused
before any filesystem or network effect, and error messages name members, not values.

| Member | Meaning | Default |
| --- | --- | --- |
| `offering` | Storage terms (`pricing`, `checkPrice`, `info`) or `null` for a requester-only node that publishes nothing | demonstration terms, 16384-byte maxima |
| `capacity` | `bytes`, `leases`, `operations`, `resultBytes`, `perOwner` finite provider limits | 1 MiB, 64, 32, 1 MiB, 8 |
| `credits` | `thirdParty`, exact `fee`, total `issuance` and `internal` Ledger budgets | refused, 0, 1000000, 10000 |
| `control` | `connections`, `lineBytes`, `requestSeconds` for local control | 4, 1 MiB, 60 |

`perOwner` cannot exceed `operations`; the advertised object maxima cannot exceed
16384 bytes in this finite profile; the check price must be positive. The Board and
Storage owners validate the offering terms again when the node starts.
