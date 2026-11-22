# Messaging state and constants

Service `cr2se.messaging`, version 1, pricing model `cr2se.messaging.v1`. **Encoding**: `opaque` (code 0x00) or `cr2se.identity-encrypted.v1`
(code 0x01); `parse` accepts those names exactly and nothing else. The smallest encrypted payload is 105 bytes.
**State**: `receiving -> pending -> recovered | removed | expired`. `may_become` allows only those transitions; terminal states never
move; `is_active` is true for pending and recovered; status can report only `pending` and `recovered`.
