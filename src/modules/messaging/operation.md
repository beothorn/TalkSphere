# Messaging operations

Logical inputs and outputs, with exact integer widths: **store** (`StoreInput`: signed message and retention; `StoreOutput`: placement ID,
`acceptedAt`, `expiresAt`, price), **discover** (`DiscoverInput`: optional pair cursor, limit; `DiscoverOutput`: entries ordered by
`(acceptedAt, placementId)`, `more`, price; an empty page is a charged success), **recover** (`RecoverInput`: placement, message, hash, recovery ID,
recipient key and signature; `RecoverOutput`: the original message and a `Receipt` with recovery ID, time, both signatures and the recipient key),
**status** (`StatusInput`; `StatusOutput` with pending or recovered state and the receipt), **renew** (placement, expected expiry, additional seconds;
previous and new expiry, price), **remove** (placement, removal ID, expected expiry, minimum release credits; release credits and price) and the paid
**check** (placement and offset; one byte). Validation of each input is the next step; these types carry no wire encoding.
