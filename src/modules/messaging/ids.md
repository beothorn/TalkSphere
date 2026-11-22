# Messaging identifiers

`MessageId`, `PlacementId`, `RecoveryId`, `RemovalId` and `PayloadHash` are exactly 32 bytes; `Signature` is exactly 64. `from_slice` accepts
only that length and returns `Invalid` otherwise. They compare and order by their bytes (placement IDs order discovery pages). Their
`Debug` forms never show bytes. None of them is a key, password or authorization: knowing one grants nothing. Constants: `ID_BYTES = 32`,
`SIGNATURE_BYTES = 64`.
