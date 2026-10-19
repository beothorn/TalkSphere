# Library boundary

The library exports `cli` for executable argument dispatch and `modules` for
protocol implementations. Importing either starts no sockets or background work.

`composition` exports explicit real-provider adapters between module contracts.
